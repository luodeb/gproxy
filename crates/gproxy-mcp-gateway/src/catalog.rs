//! The tool catalogue one API key can see.
//!
//! A key's `tools/list` is the union of every upstream server it was granted,
//! filtered per tool by its patterns. Names are the upstream's own — a grant
//! pattern like `web_*` only means something against real names — so when two
//! servers advertise the same name the first (lowest id) owns it and later ones
//! are skipped with a warning.

use std::collections::HashMap;

use gproxy_store::records::ControlSnapshot;
use rmcp::model::Tool;

pub struct ToolCatalog {
    pub tools: Vec<Tool>,
    /// Tool name → the server that serves it, for routing a call.
    pub owner: HashMap<String, i64>,
}

impl ToolCatalog {
    pub fn build(snapshot: &ControlSnapshot, permitted: impl Fn(i64, Option<&str>) -> bool) -> Self {
        let mut tools = Vec::new();
        let mut owner = HashMap::new();
        for server in snapshot
            .mcp_servers
            .iter()
            .filter(|server| server.enabled)
            .filter(|server| permitted(server.id, None))
        {
            for cached in &server.tools {
                if !permitted(server.id, Some(&cached.name)) {
                    continue;
                }
                if owner.contains_key(&cached.name) {
                    tracing::warn!(
                        tool = %cached.name,
                        server = %server.name,
                        "two MCP servers advertise the same tool name; keeping the first"
                    );
                    continue;
                }
                // Served from the discovered descriptor, so a client sees the
                // real schema without the gateway calling upstream to list.
                let schema = cached
                    .input_schema
                    .as_object()
                    .cloned()
                    .unwrap_or_else(|| serde_json::Map::from_iter([(
                        "type".to_string(),
                        serde_json::Value::String("object".into()),
                    )]));
                tools.push(Tool::new_with_raw(
                    cached.name.clone(),
                    cached.description.clone().map(Into::into),
                    std::sync::Arc::new(schema),
                ));
                owner.insert(cached.name.clone(), server.id);
            }
        }
        Self { tools, owner }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gproxy_store::records::McpServerRecord;

    fn snapshot(servers: Vec<McpServerRecord>) -> ControlSnapshot {
        ControlSnapshot {
            organizations: Vec::new(),
            teams: Vec::new(),
            providers: Vec::new(),
            credentials: Vec::new(),
            routes: Vec::new(),
            route_members: Vec::new(),
            aliases: Vec::new(),
            exposed_models: Vec::new(),
            provider_models: Vec::new(),
            mcp_servers: servers,
            users: Vec::new(),
            user_keys: Vec::new(),
            permissions: Vec::new(),
            rate_limits: Vec::new(),
            quotas: Vec::new(),
            price_rules: Vec::new(),
            price_rates: Vec::new(),
            settings: Vec::new(),
            routing_rules: Vec::new(),
            rule_sets: Vec::new(),
            rules: Vec::new(),
            provider_rule_sets: Vec::new(),
        }
    }

    fn server(id: i64, name: &str, tools: &[&str], enabled: bool) -> McpServerRecord {
        McpServerRecord {
            id,
            name: name.into(),
            url: "https://example.test/mcp".into(),
            auth_kind: "none".into(),
            auth_header_name: None,
            enabled,
            tools: tools
                .iter()
                .map(|tool| gproxy_store::records::McpToolRecord {
                    name: (*tool).to_string(),
                    description: None,
                    input_schema: serde_json::json!({"type": "object"}),
                })
                .collect(),
            tools_fetched_at: Some(0),
        }
    }

    #[test]
    fn merges_visible_servers_and_filters_by_pattern() {
        let snapshot = snapshot(vec![
            server(1, "reader", &["webReader", "webSearch"], true),
            server(2, "search", &["web_search_prime"], true),
            server(3, "disabled", &["hidden"], false),
        ]);
        let catalog = ToolCatalog::build(&snapshot, |server, tool| match (server, tool) {
            (1, _) => tool.is_none_or(|tool| tool.starts_with("web")),
            (2, _) => true,
            _ => false,
        });
        let mut names = catalog.tools.iter().map(|tool| tool.name.to_string()).collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, ["webReader", "webSearch", "web_search_prime"]);
        assert_eq!(catalog.owner.get("web_search_prime"), Some(&2));
    }

    #[test]
    fn a_server_with_no_grant_contributes_nothing() {
        let snapshot = snapshot(vec![server(1, "reader", &["webReader"], true)]);
        let catalog = ToolCatalog::build(&snapshot, |_, _| false);
        assert!(catalog.tools.is_empty());
        assert!(catalog.owner.is_empty());
    }

    #[test]
    fn the_first_server_keeps_a_contested_name() {
        let snapshot = snapshot(vec![
            server(1, "first", &["search"], true),
            server(2, "second", &["search", "other"], true),
        ]);
        let catalog = ToolCatalog::build(&snapshot, |_, _| true);
        assert_eq!(catalog.owner.get("search"), Some(&1));
        assert_eq!(catalog.owner.get("other"), Some(&2));
        assert_eq!(catalog.tools.len(), 2);
    }
}
