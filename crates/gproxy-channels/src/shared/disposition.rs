//! How an upstream status maps onto a failover decision.
//!
//! The two policies differ on one wire fact: whether the provider reports a
//! dead credential with 402/403, or reserves those for per-request refusals a
//! live credential can still hit. Picking the wrong one either burns a working
//! credential on a content-policy block, or keeps retrying a revoked one.
//!
//! A third reading splits 403 by what the body says, for providers that spend
//! the status on both kinds of refusal and name the difference in a code.

use gproxy_channel_api::{Disposition, ResponseView};
use http::StatusCode;

/// 401 only. The provider answers policy refusals, region blocks, and
/// per-request denials with 403 while the credential itself stays good.
pub(crate) fn unauthorized_only(response: ResponseView<'_>) -> Disposition {
    match response.status.as_u16() {
        200..=299 => Disposition::Success,
        401 => Disposition::CredentialDead,
        429 | 500..=599 => Disposition::Retryable,
        _ => Disposition::Terminal,
    }
}

/// 401–403. The provider reports an exhausted, suspended, or revoked
/// credential with 402 or 403 as readily as with 401.
pub(crate) fn unauthorized_or_forbidden(response: ResponseView<'_>) -> Disposition {
    match response.status.as_u16() {
        200..=299 => Disposition::Success,
        401..=403 => Disposition::CredentialDead,
        429 | 500..=599 => Disposition::Retryable,
        _ => Disposition::Terminal,
    }
}

/// Codes that name an entitlement refusal rather than a per-request one.
///
/// Aliyun Bailian answers "this key may not use this model at all" with 403 and
/// a scoped `AccessDenied` code — `AccessDenied.Unpurchased` when the model was
/// never bought for the account. Nothing a client can change alters that
/// answer: not a retry, not a rewritten prompt, not another model on the same
/// key. The credential is done for that model.
const ENTITLEMENT_DENIAL_CODES: &[&str] = &["AccessDenied"];

/// [`unauthorized_only`], except that a 403 naming an entitlement refusal is
/// read as a dead credential.
///
/// The conservative reading is right for the per-request denials a live
/// credential can still hit, but it also leaves a key that nothing will ever
/// satisfy looking healthy — and a healthy credential is neither failed over
/// nor deprecated, so the provider stays broken while the pool reports green.
pub(crate) fn unauthorized_or_unentitled(response: ResponseView<'_>) -> Disposition {
    if response.status == StatusCode::FORBIDDEN && entitlement_denied(response.body) {
        return Disposition::CredentialDead;
    }
    unauthorized_only(response)
}

/// Reads the code out of either envelope these providers use: the OpenAI
/// compatible `{"error":{"code":…,"type":…}}` and the Anthropic compatible
/// `{"code":…}`.
fn entitlement_denied(body: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
        return false;
    };
    ["code", "type"]
        .into_iter()
        .flat_map(|field| {
            [
                value.pointer(&format!("/error/{field}")),
                value.pointer(&format!("/{field}")),
            ]
        })
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .any(is_entitlement_denial)
}

fn is_entitlement_denial(code: &str) -> bool {
    ENTITLEMENT_DENIAL_CODES.iter().any(|known| {
        let Some(scope) = code.strip_prefix(known) else {
            return false;
        };
        scope.is_empty() || scope.starts_with('.')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNPURCHASED_OPENAI: &[u8] = br#"{"error":{"message":"Access to model denied. Please make sure you are eligible for using the model.","type":"AccessDenied.Unpurchased","code":"AccessDenied.Unpurchased"}}"#;
    const UNPURCHASED_CLAUDE: &[u8] = br#"{"code":"AccessDenied.Unpurchased","message":"Access to model denied. Please make sure you are eligible for using the model.","request_id":"25ba9b42-5f55-4f66-ab41-9a41b3eb3285"}"#;

    fn view<'a>(headers: &'a http::HeaderMap, status: u16, body: &'a [u8]) -> ResponseView<'a> {
        ResponseView {
            status: StatusCode::from_u16(status).expect("test status"),
            headers,
            body,
        }
    }

    #[test]
    fn unentitled_model_is_a_dead_credential() {
        let headers = http::HeaderMap::new();
        for body in [UNPURCHASED_OPENAI, UNPURCHASED_CLAUDE] {
            assert_eq!(
                unauthorized_or_unentitled(view(&headers, 403, body)),
                Disposition::CredentialDead
            );
        }
        assert_eq!(
            unauthorized_or_unentitled(view(&headers, 403, br#"{"code":"AccessDenied"}"#)),
            Disposition::CredentialDead
        );
    }

    #[test]
    fn other_forbidden_stays_terminal() {
        let headers = http::HeaderMap::new();
        for body in [
            b"{}".as_slice(),
            b"not json".as_slice(),
            br#"{"error":{"code":"misalignment_policy_violation"}}"#.as_slice(),
            br#"{"code":"AccessDeniedLite"}"#.as_slice(),
            br#"{"different":{"code":"AccessDenied"}}"#.as_slice(),
        ] {
            assert_eq!(
                unauthorized_or_unentitled(view(&headers, 403, body)),
                Disposition::Terminal
            );
        }
    }

    #[test]
    fn other_statuses_keep_the_unauthorized_only_reading() {
        let headers = http::HeaderMap::new();
        for (status, expected) in [
            (200, Disposition::Success),
            (401, Disposition::CredentialDead),
            (403, Disposition::Terminal),
            (404, Disposition::Terminal),
            (429, Disposition::Retryable),
            (500, Disposition::Retryable),
        ] {
            assert_eq!(
                unauthorized_or_unentitled(view(&headers, status, b"{}")),
                expected,
                "status {status}"
            );
        }
    }
}
