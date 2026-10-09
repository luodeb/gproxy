use gproxy_core::{CacheBackend, Core, ExecOutcome, RequestCtx};

use crate::host::AppHost;
use crate::{AppError, Shared};

pub struct App;

#[derive(Clone)]
pub struct AppHandle {
    pub(crate) inner: Shared<AppInner>,
}

/// Removes the registry entry however the request ends — result, error or
/// panic — so the live panel can never leak a finished request.
struct InFlightGuard<'a> {
    registry: &'a std::sync::Mutex<std::collections::HashMap<String, crate::InFlightEntry>>,
    request_id: String,
}

impl Drop for InFlightGuard<'_> {
    fn drop(&mut self) {
        self.registry
            .lock()
            .expect("in-flight registry poisoned")
            .remove(&self.request_id);
    }
}

/// One request currently moving through [`AppHandle::execute`].
#[derive(Debug, Clone)]
pub struct InFlightEntry {
    pub user_key_id: Option<i64>,
    pub model: Option<String>,
    pub started_at_ms: i64,
}

/// Best-effort model name from the head of a request body. Chat-shaped
/// requests carry `"model"` in their first bytes, so a bounded scan answers
/// without parsing payloads that can be megabytes.
fn peek_model(body: &bytes::Bytes) -> Option<String> {
    let head = &body[..body.len().min(16 * 1024)];
    let needle = b"\"model\"";
    let at = head
        .windows(needle.len())
        .position(|window| window == needle)?;
    let rest = &head[at + needle.len()..];
    let after = rest.iter().position(|byte| *byte == b':')?;
    let rest = &rest[after + 1..];
    let start = rest.iter().position(|byte| !byte.is_ascii_whitespace())?;
    if rest.get(start) != Some(&b'"') {
        return None;
    }
    let rest = &rest[start + 1..];
    let end = rest.iter().position(|byte| *byte == b'"')?;
    if end == 0 {
        return None;
    }
    String::from_utf8(rest[..end].to_vec()).ok()
}

pub(crate) struct AppInner {
    pub core: Core<AppHost>,
    pub host: AppHost,
    pub invalidation_version: std::sync::atomic::AtomicI64,
    /// Gateway requests admitted but not finished, with what the live panel
    /// shows about each: which key, which model, since when.
    pub in_flight: std::sync::Mutex<std::collections::HashMap<String, InFlightEntry>>,
    #[cfg(not(target_arch = "wasm32"))]
    pub shutdown: tokio::sync::watch::Sender<bool>,
    #[cfg(not(target_arch = "wasm32"))]
    pub runtime_updates:
        tokio::sync::watch::Sender<std::sync::Arc<gproxy_admin::dto::RuntimeSettingsStatusDto>>,
    #[cfg(target_arch = "wasm32")]
    pub shutdown: std::sync::atomic::AtomicBool,
}

impl AppHandle {
    pub async fn oauth_dispatch(
        &self,
        parts: &http::request::Parts,
        body: bytes::Bytes,
    ) -> Option<http::Response<bytes::Bytes>> {
        crate::oauth::dispatch(&self.inner.host, parts, body).await
    }

    pub async fn admin_dispatch(
        &self,
        parts: &http::request::Parts,
        body: bytes::Bytes,
    ) -> Option<http::Response<bytes::Bytes>> {
        let response = gproxy_admin::dispatch(self, parts, body).await;
        if response
            .as_ref()
            .is_some_and(|response| response.status().is_success())
        {
            crate::quota_refresh::opportunistic(self).await;
        }
        response
    }

    pub async fn portal_dispatch(
        &self,
        parts: &http::request::Parts,
        body: bytes::Bytes,
    ) -> Option<http::Response<bytes::Bytes>> {
        gproxy_admin::portal_dispatch(self, parts, body).await
    }

    /// Requests currently moving through the gateway, oldest first.
    pub fn in_flight_requests(&self) -> Vec<(String, crate::InFlightEntry)> {
        let registry = self
            .inner
            .in_flight
            .lock()
            .expect("in-flight registry poisoned");
        let mut entries: Vec<_> = registry
            .iter()
            .map(|(request_id, entry)| (request_id.clone(), entry.clone()))
            .collect();
        entries.sort_by_key(|(_, entry)| entry.started_at_ms);
        entries
    }

    /// Requests currently moving through the gateway.
    pub fn in_flight(&self) -> u64 {
        self.in_flight_requests().len() as u64
    }

    pub async fn execute(
        &self,
        request: RequestCtx,
    ) -> Result<ExecOutcome, gproxy_core::CoreError> {
        let entry = crate::InFlightEntry {
            user_key_id: crate::host::authenticate_headers(&self.inner.host, &request.headers)
                .ok()
                .map(|identity| identity.user_key_id),
            model: peek_model(&request.body),
            started_at_ms: web_time::SystemTime::now()
                .duration_since(web_time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis().try_into().unwrap_or(i64::MAX))
                .unwrap_or_default(),
        };
        self.inner
            .in_flight
            .lock()
            .expect("in-flight registry poisoned")
            .insert(request.request_id.clone(), entry);
        let _in_flight = InFlightGuard {
            registry: &self.inner.in_flight,
            request_id: request.request_id.clone(),
        };
        self.execute_inner(request).await
    }

    async fn execute_inner(
        &self,
        request: RequestCtx,
    ) -> Result<ExecOutcome, gproxy_core::CoreError> {
        let capture = crate::logging::begin(&self.inner.host, &request).await;
        let mut result = self
            .inner
            .core
            .execute(&self.inner.host.services.control, request)
            .await;
        if let Some(capture) = capture {
            crate::logging::finish(&self.inner.host, capture, &mut result).await;
        }
        if result.is_ok() {
            crate::quota_refresh::opportunistic(self).await;
        }
        result
    }

    pub async fn mutate(
        &self,
        mutation: crate::ControlMutation,
    ) -> Result<crate::MutationResult, AppError> {
        crate::control::apply(self, mutation).await
    }

    pub async fn reload(&self) -> Result<(), AppError> {
        let version = crate::invalidation::bump(&self.inner.host.services.cache).await;
        self.reload_local().await?;
        let version = version?;
        self.inner
            .invalidation_version
            .store(version, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    pub async fn sync_invalidation(&self) -> Result<(), AppError> {
        let version = crate::invalidation::current(&self.inner.host.services.cache).await?;
        if version
            == self
                .inner
                .invalidation_version
                .load(std::sync::atomic::Ordering::Acquire)
        {
            return Ok(());
        }
        self.reload_local().await?;
        self.inner
            .invalidation_version
            .store(version, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    async fn reload_local(&self) -> Result<(), AppError> {
        self.inner.host.services.control.reload().await?;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let settings = self.inner.host.services.control.settings();
            self.inner
                .host
                .services
                .transport
                .set_inherit_system_proxy(settings.runtime.effective.inherit_system_proxy);
            self.inner
                .host
                .services
                .transport
                .set_default_proxy(settings.runtime.effective.proxy.clone());
            self.inner
                .host
                .services
                .spawner
                .set_max_in_flight(settings.runtime.effective.max_in_flight as usize);
            let tokenizers = &self.inner.host.services.tokenizers;
            tokenizers.set_vocabs_enabled(settings.enable_tokenizer_vocabs);
            tokenizers.set_download_enabled(settings.enable_tokenizer_download);
            tokenizers.set_default_vocab(settings.default_tokenizer_vocab.clone());
            tokenizers.set_hugging_face_token(
                crate::host::tokenizers::hugging_face_token(
                    &self.inner.host.services.store,
                    &self.inner.host.services.cipher,
                )
                .await?,
            );
            self.inner.runtime_updates.send_replace(settings.runtime);
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn subscribe_runtime_settings(
        &self,
    ) -> tokio::sync::watch::Receiver<std::sync::Arc<gproxy_admin::dto::RuntimeSettingsStatusDto>>
    {
        self.inner.runtime_updates.subscribe()
    }

    pub fn file_upload_max_in_flight(&self) -> usize {
        self.inner
            .host
            .services
            .control
            .settings()
            .runtime
            .effective
            .file_upload_max_in_flight as usize
    }

    pub fn runtime_settings(&self) -> std::sync::Arc<gproxy_admin::dto::RuntimeSettingsStatusDto> {
        self.inner.host.services.control.runtime_settings()
    }

    pub fn instance_name(&self) -> String {
        self.inner.host.services.control.settings().instance_name
    }

    pub fn update_channel(&self) -> Option<String> {
        self.inner.host.services.control.settings().update_channel
    }

    pub fn shutdown(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.inner.shutdown.send_replace(true);
        #[cfg(target_arch = "wasm32")]
        self.inner
            .shutdown
            .store(true, std::sync::atomic::Ordering::Release);
    }

    pub async fn wait_shutdown(&self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut receiver = self.inner.shutdown.subscribe();
            while !*receiver.borrow_and_update() && receiver.changed().await.is_ok() {}
        }
        #[cfg(target_arch = "wasm32")]
        while !self
            .inner
            .shutdown
            .load(std::sync::atomic::Ordering::Acquire)
        {
            gloo_timers::future::TimeoutFuture::new(25).await;
        }
    }

    pub async fn admission_pending(&self, request_id: &str) -> Result<bool, AppError> {
        self.inner
            .host
            .services
            .cache
            .get(&format!("gproxy:admission:{request_id}"))
            .await
            .map(|value| value.is_some())
            .map_err(|error| AppError::Cache(error.to_string()))
    }

    pub async fn usage_by_request(
        &self,
        request_id: &str,
    ) -> Result<Option<gproxy_store::records::UsageRecord>, AppError> {
        Ok(self
            .inner
            .host
            .services
            .store
            .usage_by_request(request_id)
            .await?)
    }

    pub async fn quota_windows(
        &self,
    ) -> Result<Vec<gproxy_store::records::QuotaWindowRecord>, AppError> {
        Ok(self.inner.host.services.store.quota_windows().await?)
    }

    pub async fn observe_credential_quota_cycle(
        &self,
        observation: gproxy_store::records::CredentialQuotaObservation,
    ) -> Result<gproxy_store::records::CredentialQuotaCycleRecord, AppError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let control = self.inner.host.services.control.clone();
            tokio::spawn(async move { control.observe_credential_quota_cycle(&observation).await })
                .await
                .map_err(|error| AppError::Control(format!("quota observation task: {error}")))?
                .map_err(AppError::from)
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.inner
                .host
                .services
                .control
                .observe_credential_quota_cycle(&observation)
                .await
                .map_err(AppError::from)
        }
    }

    pub async fn close_credential_quota_cycle(
        &self,
        id: i64,
        reason: gproxy_store::records::QuotaCycleCloseReason,
        closed_at: i64,
    ) -> Result<Option<gproxy_store::records::CredentialQuotaCycleRecord>, AppError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let control = self.inner.host.services.control.clone();
            tokio::spawn(async move {
                control
                    .close_credential_quota_cycle(id, reason, closed_at)
                    .await
            })
            .await
            .map_err(|error| AppError::Control(format!("quota close task: {error}")))?
            .map_err(AppError::from)
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.inner
                .host
                .services
                .control
                .close_credential_quota_cycle(id, reason, closed_at)
                .await
                .map_err(AppError::from)
        }
    }
}

/// The end-user MCP gateway reads the same control plane as the rest of the
/// gateway, from outside the crate. These accessors are the seam: an upstream
/// service resolves a caller, checks one grant, and opens one server credential.
#[cfg(not(target_arch = "wasm32"))]
impl AppHandle {
    /// Resolve a caller from request headers, exactly as `/v1` does.
    pub fn mcp_identity(
        &self,
        headers: &http::HeaderMap,
    ) -> Result<gproxy_channel_api::CallerIdentity, gproxy_core::CoreError> {
        crate::host::authenticate_headers(&self.inner.host, headers)
    }

    pub fn mcp_snapshot(
        &self,
    ) -> std::sync::Arc<gproxy_store::records::ControlSnapshot> {
        self.inner.host.services.control.current()
    }

    /// Whether this caller may reach `server_id`, and `tool` on it.
    pub fn mcp_permitted(
        &self,
        identity: &gproxy_channel_api::CallerIdentity,
        server_id: i64,
        tool: Option<&str>,
    ) -> bool {
        crate::host::mcp_permitted(&self.mcp_snapshot(), identity, server_id, tool)
    }

    /// The upstream credential of one server, ready for the MCP client. `None`
    /// means the server needs no auth; a server whose credential is missing or
    /// unreadable is reported as an internal error rather than silently called
    /// without one.
    pub async fn mcp_auth(
        &self,
        server_id: i64,
    ) -> Result<Option<gproxy_upstream::mcp_client::UpstreamMcpAuth>, gproxy_core::CoreError> {
        use gproxy_upstream::mcp_client::UpstreamMcpAuth;
        let services = &self.inner.host.services;
        let Some(server) = services.store.mcp_server(server_id).await.map_err(internal)? else {
            return Err(gproxy_core::CoreError::Internal(format!(
                "unknown mcp server {server_id}"
            )));
        };
        match server.auth_kind.as_str() {
            "none" => Ok(None),
            "bearer" | "header" => {
                let secret = services
                    .store
                    .mcp_server_secret(server_id)
                    .await
                    .map_err(internal)?
                    .and_then(|record| record.envelope)
                    .ok_or_else(|| {
                        gproxy_core::CoreError::Internal(format!(
                            "mcp server {server_id} has no stored credential"
                        ))
                    })?;
                let opened = services.cipher.open(&secret).map_err(internal)?;
                let token = opened
                    .get("token")
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        gproxy_core::CoreError::Internal(format!(
                            "mcp server {server_id} credential has no token"
                        ))
                    })?;
                Ok(Some(if server.auth_kind == "bearer" {
                    UpstreamMcpAuth::Bearer(token.to_owned())
                } else {
                    UpstreamMcpAuth::Header(
                        server.auth_header_name.unwrap_or_else(|| "authorization".into()),
                        token.to_owned(),
                    )
                }))
            }
            other => Err(gproxy_core::CoreError::Internal(format!(
                "mcp server {server_id} has an unsupported auth kind: {other}"
            ))),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn internal(error: impl std::fmt::Display) -> gproxy_core::CoreError {
    gproxy_core::CoreError::Internal(error.to_string())
}
