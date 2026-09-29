//! Description of the admin API surface the MCP tools drive.
//!
//! The catalog is the single source of truth behind two things: the
//! `gproxy_endpoints` tool, which lets an agent discover what it can call, and
//! the generated OpenAPI document served at [`crate::OPENAPI_PATH`].
//!
//! Keeping the two in sync by construction matters because the admin API is
//! hand-routed: there is no framework metadata to introspect.
//!
//! Note that every entry below is a plain HTTP call. The catalog does not
//! restate request or response schemas — the admin API validates those and
//! reports failures per entity, and duplicating two hundred DTOs here would go
//! stale silently.

/// One callable admin API operation.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Endpoint {
    /// Upper-case HTTP method.
    pub method: &'static str,
    /// Path template; `{...}` marks a path parameter.
    pub path: &'static str,
    /// One-line description of what the call does.
    pub summary: &'static str,
    /// CRUD entity this endpoint belongs to, when it is a plain entity route.
    pub entity: Option<&'static str>,
}

impl Endpoint {
    pub(crate) const fn new(
        method: &'static str,
        path: &'static str,
        summary: &'static str,
        entity: Option<&'static str>,
    ) -> Self {
        Self {
            method,
            path,
            summary,
            entity,
        }
    }

    /// Whether the operation can modify state.
    pub(crate) const fn is_write(&self) -> bool {
        !matches!(self.method.as_bytes(), b"GET" | b"HEAD")
    }

    /// Path parameters named in the template, in order of appearance.
    pub(crate) fn parameters(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        let mut rest = self.path;
        while let Some(start) = rest.find('{') {
            let after = &rest[start + 1..];
            let Some(end) = after.find('}') else { break };
            names.push(&after[..end]);
            rest = &after[end + 1..];
        }
        names
    }
}

/// Entities addressable as `GET|POST /admin/api/<entity>`,
/// `PATCH|DELETE /admin/api/<entity>/{id}` and
/// `POST /admin/api/batch/<entity>`, paired with a human-readable label.
pub(crate) const ENTITY_LABELS: &[(&str, &str)] = &[
    ("organizations", "organizations"),
    ("teams", "teams"),
    ("providers", "providers"),
    ("credentials", "credentials"),
    ("routes", "routes"),
    ("route-members", "route members"),
    ("aliases", "aliases"),
    ("model-aliases", "model aliases"),
    ("provider-models", "provider models"),
    ("users", "users"),
    ("user-keys", "user keys"),
    ("permissions", "permissions"),
    ("rate-limits", "rate limits"),
    ("quotas", "quotas"),
    ("price-rules", "price rules"),
    ("price-rates", "price rates"),
    ("routing-rules", "routing rules"),
    ("rule-sets", "rule sets"),
    ("rules", "rules"),
    ("provider-rule-sets", "provider rule sets"),
];

/// Batch actions the admin API accepts.
pub(crate) const BATCH_ACTIONS: &[&str] = &["enable", "disable", "delete"];

/// Human-readable label for an entity.
pub(crate) fn entity_label(entity: &str) -> Option<&'static str> {
    ENTITY_LABELS
        .iter()
        .find_map(|(name, label)| (*name == entity).then_some(*label))
}

/// True when `entity` is an entity addressable through the generic tools.
pub(crate) fn is_entity(entity: &str) -> bool {
    entity_label(entity).is_some()
}

/// Build the catalog: the generic entity routes followed by the special ends.
///
/// The entity repetition is expanded inline rather than delegated to a helper
/// macro: a `macro_rules!` invocation in expression position must expand to a
/// single expression, and each entity expands to five comma-separated ones.
macro_rules! catalog {
    (
        entities: [$(($entity:literal, $label:literal)),* $(,)?],
        extra: [$(($method:literal, $path:literal, $summary:literal $(,)?)),* $(,)?],
    ) => {
        &[
            $(
                Endpoint::new(
                    "GET",
                    concat!("/admin/api/", $entity),
                    concat!("List ", $label, "."),
                    Some($entity),
                ),
                Endpoint::new(
                    "POST",
                    concat!("/admin/api/", $entity),
                    concat!("Create one ", $label, " record."),
                    Some($entity),
                ),
                Endpoint::new(
                    "PATCH",
                    concat!("/admin/api/", $entity, "/{id}"),
                    concat!(
                        "Update one ",
                        $label,
                        " record; the body replaces the stored value, so send the full object."
                    ),
                    Some($entity),
                ),
                Endpoint::new(
                    "DELETE",
                    concat!("/admin/api/", $entity, "/{id}"),
                    concat!("Delete one ", $label, " record."),
                    Some($entity),
                ),
                Endpoint::new(
                    "POST",
                    concat!("/admin/api/batch/", $entity),
                    concat!("Apply enable, disable or delete to many ", $label, " records at once."),
                    Some($entity),
                ),
            )*
            $(Endpoint::new($method, $path, $summary, None),)*
        ]
    };
}

/// The complete admin API catalog.
pub(crate) const ENDPOINTS: &[Endpoint] = catalog! {
    entities: [
        ("organizations", "organizations"),
        ("teams", "teams"),
        ("providers", "providers"),
        ("credentials", "credentials"),
        ("routes", "routes"),
        ("route-members", "route members"),
        ("aliases", "aliases"),
        ("model-aliases", "model aliases"),
        ("provider-models", "provider models"),
        ("users", "users"),
        ("user-keys", "user keys"),
        ("permissions", "permissions"),
        ("rate-limits", "rate limits"),
        ("quotas", "quotas"),
        ("price-rules", "price rules"),
        ("price-rates", "price rates"),
        ("routing-rules", "routing rules"),
        ("rule-sets", "rule sets"),
        ("rules", "rules"),
        ("provider-rule-sets", "provider rule sets"),
    ],
    extra: [
        ("GET", "/admin/api/session", "Report the setup state and current admin session."),
        ("GET", "/admin/api/oauth-clients", "List OAuth clients."),
        ("POST", "/admin/api/oauth-clients", "Create an OAuth client."),
        ("PATCH", "/admin/api/oauth-clients/{id}", "Update an OAuth client."),
        ("DELETE", "/admin/api/oauth-clients/{id}", "Delete an OAuth client."),
        ("GET", "/admin/api/usage", "Aggregate usage for a time range, optionally grouped."),
        ("GET", "/admin/api/usage-records", "Page through individual usage records."),
        ("GET", "/admin/api/usage-summary", "Summarise usage for a time range."),
        ("GET", "/admin/api/usage-trend", "Usage trend over a time range."),
        ("GET", "/admin/api/quota-windows", "List quota windows across credentials."),
        ("GET", "/admin/api/credential-cycles", "List credential quota cycles."),
        ("GET", "/admin/api/channels", "List channel types this build supports."),
        ("GET", "/admin/api/tls-presets", "List TLS fingerprint presets."),
        ("GET", "/admin/api/rule-presets", "List routing rule presets."),
        ("GET", "/admin/api/audit", "List audit entries, newest first."),
        ("GET", "/admin/api/logs", "List request log entries, newest first."),
        (
            "GET",
            "/admin/api/logs/{request_id}",
            "Fetch the downstream and upstream record of one request.",
        ),
        ("GET", "/admin/api/log-settings", "Read request log capture settings."),
        ("PATCH", "/admin/api/log-settings", "Update request log capture settings."),
        ("GET", "/admin/api/instance-settings", "Read instance settings."),
        ("PATCH", "/admin/api/instance-settings", "Update instance settings."),
        ("GET", "/admin/api/tokenizer-vocabs", "List installed tokenizer vocabularies."),
        ("POST", "/admin/api/tokenizer-vocabs", "Download a tokenizer vocabulary."),
        ("DELETE", "/admin/api/tokenizer-vocabs", "Remove a tokenizer vocabulary."),
        (
            "GET",
            "/admin/api/tokenizer-vocabs/progress",
            "Report tokenizer download progress.",
        ),
        (
            "GET",
            "/admin/api/tokenizer-auth",
            "Report whether a tokenizer registry token is configured.",
        ),
        (
            "PATCH",
            "/admin/api/tokenizer-auth",
            "Set or clear the tokenizer registry token.",
        ),
        (
            "POST",
            "/admin/api/tokenizer-auth/reveal",
            "Reveal the stored tokenizer registry token.",
        ),
        ("GET", "/admin/api/portal-settings", "Read portal settings."),
        ("PATCH", "/admin/api/portal-settings", "Update portal settings."),
        ("POST", "/admin/api/export", "Export the whole configuration, optionally with secrets."),
        ("POST", "/admin/api/import", "Import a previously exported configuration."),
        ("GET", "/admin/api/default-model-catalog", "List the built-in default model catalog."),
        (
            "POST",
            "/admin/api/default-model-catalog/apply-prices",
            "Apply catalog prices to a provider's models.",
        ),
        ("GET", "/admin/api/price-catalog", "List the built-in price catalog."),
        ("POST", "/admin/api/connectivity/test", "Run an egress connectivity probe."),
        (
            "POST",
            "/admin/api/models/test",
            "Send one completion through a provider to verify it works.",
        ),
        ("POST", "/admin/api/models/discover", "Ask a provider which models it serves."),
        (
            "GET",
            "/admin/api/credentials/{id}/quota",
            "Read the cached quota snapshot of a credential.",
        ),
        (
            "POST",
            "/admin/api/credentials/{id}/quota-probe",
            "Probe the upstream subscription usage of a credential.",
        ),
        (
            "POST",
            "/admin/api/credentials/{id}/quota-reset",
            "Redeem a quota reset credit for a credential.",
        ),
        (
            "POST",
            "/admin/api/credentials/{id}/health-reset",
            "Clear the failure state of a credential.",
        ),
        (
            "POST",
            "/admin/api/credentials/{id}/reveal",
            "Reveal a credential's stored secret.",
        ),
        ("POST", "/admin/api/users/{id}/password", "Set a user's password."),
        ("POST", "/admin/api/user-keys/{id}/reveal", "Reveal a user key's secret."),
        (
            "POST",
            "/admin/api/rule-sets/{id}/rule-presets/{preset}",
            "Apply a preset to a rule set.",
        ),
        (
            "POST",
            "/admin/api/providers/{id}/routing-defaults/reset",
            "Reset a provider's routing defaults.",
        ),
        ("POST", "/admin/api/login/authcode/start", "Start an authorization-code channel login."),
        (
            "POST",
            "/admin/api/login/authcode/complete",
            "Complete an authorization-code channel login.",
        ),
        ("POST", "/admin/api/login/device/start", "Start a device-code channel login."),
        ("POST", "/admin/api/login/device/poll", "Poll a device-code channel login."),
        ("POST", "/admin/api/login/cookie", "Exchange a pasted cookie for channel credentials."),
    ],
};

#[cfg(test)]
mod tests {
    use super::{BATCH_ACTIONS, ENDPOINTS, ENTITY_LABELS, Endpoint, entity_label, is_entity};

    #[test]
    fn every_entity_has_the_full_crud_surface() {
        for (entity, _) in ENTITY_LABELS {
            for method in ["GET", "POST", "PATCH", "DELETE"] {
                let base = format!("/admin/api/{entity}");
                assert!(
                    ENDPOINTS.iter().any(|endpoint| {
                        endpoint.method == method
                            && (endpoint.path == base || endpoint.path == format!("{base}/{{id}}"))
                    }),
                    "{method} {base} missing from the catalog"
                );
            }
            assert!(
                ENDPOINTS
                    .iter()
                    .any(|endpoint| endpoint.path == format!("/admin/api/batch/{entity}")),
                "batch {entity} missing from the catalog"
            );
        }
    }

    #[test]
    fn catalog_entries_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for endpoint in ENDPOINTS {
            assert!(
                seen.insert((endpoint.method, endpoint.path)),
                "duplicate catalog entry for {} {}",
                endpoint.method,
                endpoint.path
            );
        }
    }

    #[test]
    fn catalog_paths_are_admin_routes() {
        for endpoint in ENDPOINTS {
            assert!(
                endpoint.path.starts_with("/admin/api/"),
                "{} is not an admin API route",
                endpoint.path
            );
            assert!(
                !endpoint.path.ends_with('/'),
                "{} should not end with a slash",
                endpoint.path
            );
        }
    }

    #[test]
    fn entity_lookup_agrees_with_the_table() {
        for (entity, _) in ENTITY_LABELS {
            assert!(is_entity(entity), "{entity} is not recognised");
            assert!(entity_label(entity).is_some(), "{entity} has no label");
        }
        assert!(!is_entity("nope"));
        assert_eq!(entity_label("user-keys"), Some("user keys"));
    }

    #[test]
    fn methods_and_parameters_are_classified() {
        let endpoint = Endpoint::new(
            "POST",
            "/admin/api/rule-sets/{id}/rule-presets/{preset}",
            "",
            None,
        );
        assert!(endpoint.is_write());
        assert_eq!(endpoint.parameters(), vec!["id", "preset"]);
        let read = Endpoint::new("GET", "/admin/api/providers", "", Some("providers"));
        assert!(!read.is_write());
        assert!(read.parameters().is_empty());
        assert_eq!(BATCH_ACTIONS, ["enable", "disable", "delete"]);
    }
}
