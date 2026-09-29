//! The MCP tool surface.
//!
//! Every tool is a thin, typed wrapper over the gproxy admin API. Rather than
//! one tool per endpoint — the catalog has well over a hundred — the surface is
//! a handful of aggregate tools plus [`gproxy_request`], a universal escape
//! hatch that can reach any admin route. That keeps the tool list small enough
//! for a model to hold in context while still exposing the whole control plane.
//!
//! Tools never touch the store directly. They synthesise an admin API request
//! from the caller's own HTTP request (preserving its credentials) and hand it
//! to [`gproxy_app::AppHandle::admin_dispatch`], so authentication,
//! authorization, validation, audit recording and quota bookkeeping behave
//! exactly as they do for the console.

use bytes::Bytes;
use http::header::{CONTENT_LENGTH, CONTENT_TYPE, TRANSFER_ENCODING};
use http::request::{Parts, Request};
use http::{HeaderName, HeaderValue, Method, Uri};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use rmcp::model::{CallToolResult, JsonObject, Tool, ToolAnnotations};
use rmcp::{ErrorData, model::ErrorCode};
use serde_json::{Value, json};

use crate::catalog::{self, Endpoint};

type McpResult<T> = Result<T, ErrorData>;

/// Characters left untouched when encoding a path segment: the RFC 3986
/// unreserved set. Notably this keeps `-`, `.`, `_` and `~` intact, which
/// matters for the `rule-presets/{preset}` route — the admin API does not
/// percent-decode that segment, so preset names must survive byte for byte.
const PATH_SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Characters left untouched in a query key or value. A space becomes `%20`,
/// never `+`, so values round-trip through the admin API's decoder unchanged.
const QUERY_PART: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Methods a tool may use against the admin API.
const ALLOWED_METHODS: &[&str] = &["GET", "HEAD", "POST", "PATCH", "DELETE"];

/// Prefix every tool path must live under.
const ADMIN_PREFIX: &str = "/admin/api/";

/// Path of this MCP endpoint; calling it from a tool would recurse.
const MCP_PATH: &str = crate::GATEWAY_MCP_PATH;

/// The tools advertised to clients.
pub(crate) fn tools() -> Vec<Tool> {
    vec![
        tool(
            "gproxy_endpoints",
            "Discover the gproxy admin API. Lists every callable route with its \
             method, path and purpose, optionally filtered by entity, method or a \
             search term. Use this before gproxy_request to find the right path.",
            json!({
                "type": "object",
                "properties": {
                    "entity": {
                        "type": "string",
                        "description": "Only routes belonging to this entity, e.g. providers or user-keys."
                    },
                    "method": {
                        "type": "string",
                        "description": "Only routes using this HTTP method."
                    },
                    "search": {
                        "type": "string",
                        "description": "Only routes whose path or summary contains this text."
                    }
                },
                "additionalProperties": false
            }),
            true,
            false,
        ),
        tool(
            "gproxy_openapi",
            "Fetch the OpenAPI description of the gproxy admin API. Without \
             arguments returns the whole document; pass a path to get just that \
             route's definition.",
            json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Optional route path such as /admin/api/providers."
                    }
                },
                "additionalProperties": false
            }),
            true,
            false,
        ),
        tool(
            "gproxy_request",
            "Call any gproxy admin API route directly. This is the escape hatch \
             that reaches the whole control plane when no dedicated tool fits. \
             Paths are relative to the origin and must start with /admin/api/.",
            json!({
                "type": "object",
                "properties": {
                    "method": {
                        "type": "string",
                        "enum": ALLOWED_METHODS,
                        "description": "HTTP method to use."
                    },
                    "path": {
                        "type": "string",
                        "description": "Admin API path, e.g. /admin/api/providers/3."
                    },
                    "query": {
                        "type": "object",
                        "description": "Query parameters as a flat object of scalars.",
                        "additionalProperties": { "type": ["string", "number", "boolean"] }
                    },
                    "body": {
                        "description": "Request body, serialised as JSON. Omit for GET and DELETE."
                    },
                    "headers": {
                        "type": "object",
                        "description": "Extra request headers as a flat object of strings.",
                        "additionalProperties": { "type": "string" }
                    }
                },
                "required": ["method", "path"],
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool(
            "gproxy_list",
            "List records of one entity. Returns the array the admin API serves.",
            json!({
                "type": "object",
                "properties": {
                    "entity": entity_schema(),
                    "query": {
                        "type": "object",
                        "description": "Optional query parameters as a flat object of scalars.",
                        "additionalProperties": { "type": ["string", "number", "boolean"] }
                    }
                },
                "required": ["entity"],
                "additionalProperties": false
            }),
            true,
            false,
        ),
        tool(
            "gproxy_get",
            "Fetch one record by id. The admin API has no read-by-id route, so \
             this lists the entity and filters the result client-side.",
            json!({
                "type": "object",
                "properties": {
                    "entity": entity_schema(),
                    "id": { "type": "integer", "description": "Primary key of the record." }
                },
                "required": ["entity", "id"],
                "additionalProperties": false
            }),
            true,
            false,
        ),
        tool(
            "gproxy_create",
            "Create one record of an entity. The body is the entity's object as \
             the admin API expects it; call gproxy_get on an existing record to \
             see the shape.",
            json!({
                "type": "object",
                "properties": {
                    "entity": entity_schema(),
                    "body": { "type": "object", "description": "New record." }
                },
                "required": ["entity", "body"],
                "additionalProperties": false
            }),
            false,
            false,
        ),
        tool(
            "gproxy_update",
            "Update one record. The body replaces the stored value, so send the \
             full object — read it with gproxy_get first and change only the \
             fields you mean to change.",
            json!({
                "type": "object",
                "properties": {
                    "entity": entity_schema(),
                    "id": { "type": "integer", "description": "Primary key of the record." },
                    "body": { "type": "object", "description": "Complete replacement value." }
                },
                "required": ["entity", "id", "body"],
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool(
            "gproxy_delete",
            "Delete one record by id. This is irreversible.",
            json!({
                "type": "object",
                "properties": {
                    "entity": entity_schema(),
                    "id": { "type": "integer", "description": "Primary key of the record." }
                },
                "required": ["entity", "id"],
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool(
            "gproxy_batch",
            "Apply enable, disable or delete to many records of an entity in one \
             call. Returns a per-id outcome, so partial success is visible.",
            json!({
                "type": "object",
                "properties": {
                    "entity": entity_schema(),
                    "action": {
                        "type": "string",
                        "enum": catalog::BATCH_ACTIONS,
                        "description": "Action applied to every id."
                    },
                    "ids": {
                        "type": "array",
                        "items": { "type": "integer" },
                        "description": "Primary keys to act on."
                    }
                },
                "required": ["entity", "action", "ids"],
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool(
            "gproxy_usage",
            "Read traffic usage. Choose the aggregation with scope: aggregate \
             totals, a page of individual records, a summary, or a time trend.",
            json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["aggregate", "records", "summary", "trend"],
                        "default": "aggregate",
                        "description": "Which usage view to read."
                    },
                    "from": { "type": "integer", "description": "Range start, epoch milliseconds." },
                    "to": { "type": "integer", "description": "Range end, epoch milliseconds." },
                    "group_by": {
                        "type": "string",
                        "enum": ["user_key", "user", "provider", "model"],
                        "description": "Split the aggregate per group."
                    },
                    "user_id": { "type": "integer" },
                    "user_key_id": { "type": "integer" },
                    "provider_id": { "type": "integer" },
                    "credential_id": { "type": "integer" },
                    "model": { "type": "string" },
                    "operation": { "type": "string" },
                    "request_id": { "type": "string" },
                    "usage_source": { "type": "string", "enum": ["upstream", "estimated"] },
                    "ended": { "type": "string", "enum": ["complete", "interrupted"] },
                    "page": { "type": "integer", "description": "Records page, 1-based." },
                    "page_size": {
                        "type": "integer",
                        "enum": [10, 20, 50, 100],
                        "description": "Records per page."
                    }
                },
                "required": ["from", "to"],
                "additionalProperties": false
            }),
            true,
            false,
        ),
        tool(
            "gproxy_logs",
            "Read request logs. List them, fetch one request's full downstream and \
             upstream exchange, or read and update capture settings.",
            json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["list", "detail", "settings"],
                        "default": "list",
                        "description": "Which log view to use."
                    },
                    "request_id": {
                        "type": "string",
                        "description": "Required for scope=detail."
                    },
                    "start": { "type": "integer", "description": "Range start, epoch milliseconds." },
                    "end": { "type": "integer", "description": "Range end, epoch milliseconds." },
                    "user_id": { "type": "integer" },
                    "user_key_id": { "type": "integer" },
                    "provider_id": { "type": "integer" },
                    "status": { "type": "integer", "description": "Filter by response status." },
                    "cursor": { "type": "integer", "description": "Pagination cursor from the previous page." },
                    "limit": { "type": "integer" },
                    "settings": {
                        "type": "object",
                        "description": "For scope=settings, sends a PATCH with these fields instead of reading."
                    }
                },
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool(
            "gproxy_audit",
            "List audit entries: who changed what, newest first.",
            json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum entries to return, 1 to 500."
                    }
                },
                "additionalProperties": false
            }),
            true,
            false,
        ),
        tool(
            "gproxy_settings",
            "Read or update instance-wide settings: instance, request log capture, \
             or portal behaviour.",
            json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["instance", "log", "portal"],
                        "default": "instance",
                        "description": "Which settings group to address."
                    },
                    "update": {
                        "type": "object",
                        "description": "When present, sends a PATCH with these fields instead of reading."
                    }
                },
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool(
            "gproxy_transfer",
            "Export or import the whole gateway configuration. Exports can include \
             secrets, so treat the result as sensitive.",
            json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["export", "import"],
                        "description": "Direction of the transfer."
                    },
                    "include_secrets": {
                        "type": "boolean",
                        "description": "For export: include credential and key secrets."
                    },
                    "export": {
                        "type": "object",
                        "description": "For import: a document previously returned by export."
                    },
                    "source_master_key": {
                        "type": "string",
                        "description": "For import: master key the export was sealed with, when it differs."
                    }
                },
                "required": ["action"],
                "additionalProperties": false
            }),
            false,
            true,
        ),
        tool(
            "gproxy_diagnostics",
            "Run live checks against providers and credentials, and read the \
             catalogue of channel types, TLS presets, quota windows and cycles.",
            json!({
                "type": "object",
                "properties": {
                    "check": {
                        "type": "string",
                        "enum": [
                            "connectivity",
                            "model-test",
                            "model-discover",
                            "channels",
                            "tls-presets",
                            "rule-presets",
                            "quota-windows",
                            "credential-cycles",
                            "credential-quota",
                            "credential-quota-probe",
                            "credential-quota-reset",
                            "credential-health-reset"
                        ],
                        "description": "Which check to run."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["global", "proxy", "provider", "credential"],
                        "description": "For check=connectivity: what to probe through."
                    },
                    "proxy_url": { "type": "string", "description": "For check=connectivity." },
                    "provider_id": { "type": "integer", "description": "For provider-scoped checks." },
                    "model_id": { "type": "string", "description": "For check=model-test." },
                    "credential_id": { "type": "integer", "description": "For credential-scoped checks." }
                },
                "required": ["check"],
                "additionalProperties": false
            }),
            false,
            true,
        ),
    ]
}

/// Look up one tool definition by name.
pub(crate) fn lookup(name: &str) -> Option<Tool> {
    tools().into_iter().find(|tool| tool.name == name)
}

/// Build a tool definition.
///
/// `read_only` and `destructive` become MCP annotations so a client can tell
/// safe reads from calls that replace or remove stored state before it runs
/// them. Only `GET`-shaped tools are read-only; everything else is annotated
/// destructive except `gproxy_create`, which is additive.
fn tool(
    name: &'static str,
    description: &'static str,
    schema: Value,
    read_only: bool,
    destructive: bool,
) -> Tool {
    let schema = schema.as_object().cloned().unwrap_or_default();
    let annotations = ToolAnnotations::new()
        .read_only(read_only)
        .destructive(destructive);
    Tool::new(name, description, schema).with_annotations(annotations)
}

fn entity_schema() -> Value {
    json!({
        "type": "string",
        "enum": catalog::ENTITY_LABELS.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "description": "Entity to address. Call gproxy_endpoints for the full route list."
    })
}

/// Execute one tool call.
pub(crate) async fn call(
    app: &gproxy_app::AppHandle,
    name: &str,
    arguments: Option<JsonObject>,
    caller: Option<&Parts>,
) -> McpResult<CallToolResult> {
    let args = arguments.unwrap_or_default();
    match name {
        "gproxy_endpoints" => endpoints(&args),
        "gproxy_openapi" => openapi(&args),
        "gproxy_request" => {
            let method = required_str(&args, "method")?.to_uppercase();
            let path = required_str(&args, "path")?.to_owned();
            let query = scalar_query(&args, &[]).or_else(|| nested_scalar_query(&args, "query"));
            let headers = nested_string_map(&args, "headers");
            let body = args.get("body").cloned();
            dispatch(app, caller, &method, &path, query, &body, headers.as_ref()).await
        }
        "gproxy_list" => {
            let entity = entity(&args)?;
            let query = nested_scalar_query(&args, "query");
            dispatch(
                app,
                caller,
                "GET",
                &format!("/admin/api/{entity}"),
                query,
                &None,
                None,
            )
            .await
        }
        "gproxy_get" => {
            let entity = entity(&args)?;
            let id = required_i64(&args, "id")?;
            let result = dispatch(
                app,
                caller,
                "GET",
                &format!("/admin/api/{entity}"),
                None,
                &None,
                None,
            )
            .await?;
            filter_by_id(result, id)
        }
        "gproxy_create" => {
            let entity = entity(&args)?;
            let body = args.get("body").cloned();
            dispatch(
                app,
                caller,
                "POST",
                &format!("/admin/api/{entity}"),
                None,
                &body,
                None,
            )
            .await
        }
        "gproxy_update" => {
            let entity = entity(&args)?;
            let id = required_i64(&args, "id")?;
            let body = args.get("body").cloned();
            dispatch(
                app,
                caller,
                "PATCH",
                &format!("/admin/api/{entity}/{id}"),
                None,
                &body,
                None,
            )
            .await
        }
        "gproxy_delete" => {
            let entity = entity(&args)?;
            let id = required_i64(&args, "id")?;
            dispatch(
                app,
                caller,
                "DELETE",
                &format!("/admin/api/{entity}/{id}"),
                None,
                &None,
                None,
            )
            .await
        }
        "gproxy_batch" => {
            let entity = entity(&args)?;
            let action = required_str(&args, "action")?;
            if !catalog::BATCH_ACTIONS.contains(&action) {
                return Err(ErrorData::invalid_params(
                    format!("unknown batch action `{action}`"),
                    None,
                ));
            }
            let ids = args
                .get("ids")
                .and_then(Value::as_array)
                .ok_or_else(|| ErrorData::invalid_params("ids must be an array", None))?;
            let body = Some(json!({ "action": action, "ids": ids }));
            dispatch(
                app,
                caller,
                "POST",
                &format!("/admin/api/batch/{entity}"),
                None,
                &body,
                None,
            )
            .await
        }
        "gproxy_usage" => usage(app, caller, &args).await,
        "gproxy_logs" => logs(app, caller, &args).await,
        "gproxy_audit" => {
            let query = scalar_query(&args, &["limit"]);
            dispatch(app, caller, "GET", "/admin/api/audit", query, &None, None).await
        }
        "gproxy_settings" => settings(app, caller, &args).await,
        "gproxy_transfer" => transfer(app, caller, &args).await,
        "gproxy_diagnostics" => diagnostics(app, caller, &args).await,
        other => Err(ErrorData::new(
            ErrorCode::METHOD_NOT_FOUND,
            format!("unknown tool: {other}"),
            None,
        )),
    }
}

fn endpoints(args: &JsonObject) -> McpResult<CallToolResult> {
    let entity = str_arg(args, "entity");
    let method = str_arg(args, "method").map(str::to_uppercase);
    let search = str_arg(args, "search").map(str::to_lowercase);
    let matching = catalog::ENDPOINTS
        .iter()
        .filter(|endpoint| {
            entity.is_none_or(|want| endpoint.entity == Some(want))
                && method.as_deref().is_none_or(|want| endpoint.method == want)
                && search.as_deref().is_none_or(|needle| {
                    endpoint.path.to_lowercase().contains(needle)
                        || endpoint.summary.to_lowercase().contains(needle)
                })
        })
        .map(describe)
        .collect::<Vec<_>>();
    Ok(CallToolResult::structured(json!({
        "count": matching.len(),
        "entities": catalog::ENTITY_LABELS.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        "batch_actions": catalog::BATCH_ACTIONS,
        "endpoints": matching,
    })))
}

fn describe(endpoint: &Endpoint) -> Value {
    json!({
        "method": endpoint.method,
        "path": endpoint.path,
        "summary": endpoint.summary,
        "entity": endpoint.entity,
        "write": endpoint.is_write(),
        "parameters": endpoint.parameters(),
    })
}

fn openapi(args: &JsonObject) -> McpResult<CallToolResult> {
    let document = crate::document();
    match str_arg(args, "path") {
        None => Ok(CallToolResult::structured(document)),
        Some(path) => {
            let item = document
                .get("paths")
                .and_then(|paths| paths.get(path))
                .cloned();
            match item {
                Some(item) => Ok(CallToolResult::structured(item)),
                None => Ok(CallToolResult::structured_error(json!({
                    "error": format!("no such admin route: {path}"),
                }))),
            }
        }
    }
}

async fn usage(
    app: &gproxy_app::AppHandle,
    caller: Option<&Parts>,
    args: &JsonObject,
) -> McpResult<CallToolResult> {
    let scope = str_arg(args, "scope").unwrap_or("aggregate");
    let path = match scope {
        "aggregate" => "/admin/api/usage",
        "records" => "/admin/api/usage-records",
        "summary" => "/admin/api/usage-summary",
        "trend" => "/admin/api/usage-trend",
        other => {
            return Err(ErrorData::invalid_params(
                format!("unknown usage scope `{other}`"),
                None,
            ));
        }
    };
    if args.get("from").is_none() {
        return Err(ErrorData::invalid_params("from is required", None));
    }
    if args.get("to").is_none() {
        return Err(ErrorData::invalid_params("to is required", None));
    }
    let keys = [
        "from",
        "to",
        "group_by",
        "user_id",
        "user_key_id",
        "provider_id",
        "credential_id",
        "model",
        "operation",
        "request_id",
        "usage_source",
        "ended",
        "page",
        "page_size",
    ];
    let mut query = scalar_query(args, &keys);
    if matches!(scope, "summary" | "trend") {
        // Neither route accepts the per-record filters the aggregate routes take.
        query = query.map(|pairs| {
            pairs
                .into_iter()
                .filter(|(key, _)| matches!(key.as_str(), "from" | "to"))
                .collect()
        });
    }
    dispatch(app, caller, "GET", path, query, &None, None).await
}

async fn logs(
    app: &gproxy_app::AppHandle,
    caller: Option<&Parts>,
    args: &JsonObject,
) -> McpResult<CallToolResult> {
    let scope = str_arg(args, "scope").unwrap_or("list");
    match scope {
        "list" => {
            let keys = [
                "start",
                "end",
                "user_id",
                "user_key_id",
                "provider_id",
                "status",
                "cursor",
                "limit",
            ];
            let query = scalar_query(args, &keys);
            dispatch(app, caller, "GET", "/admin/api/logs", query, &None, None).await
        }
        "detail" => {
            let request_id = required_str(args, "request_id")?;
            let path = format!("/admin/api/logs/{}", encode_segment(request_id));
            dispatch(app, caller, "GET", &path, None, &None, None).await
        }
        "settings" => match args.get("settings") {
            Some(settings) => {
                let body = Some(settings.clone());
                dispatch(
                    app,
                    caller,
                    "PATCH",
                    "/admin/api/log-settings",
                    None,
                    &body,
                    None,
                )
                .await
            }
            None => {
                dispatch(
                    app,
                    caller,
                    "GET",
                    "/admin/api/log-settings",
                    None,
                    &None,
                    None,
                )
                .await
            }
        },
        other => Err(ErrorData::invalid_params(
            format!("unknown logs scope `{other}`"),
            None,
        )),
    }
}

async fn settings(
    app: &gproxy_app::AppHandle,
    caller: Option<&Parts>,
    args: &JsonObject,
) -> McpResult<CallToolResult> {
    let scope = str_arg(args, "scope").unwrap_or("instance");
    let path = match scope {
        "instance" => "/admin/api/instance-settings",
        "log" => "/admin/api/log-settings",
        "portal" => "/admin/api/portal-settings",
        other => {
            return Err(ErrorData::invalid_params(
                format!("unknown settings scope `{other}`"),
                None,
            ));
        }
    };
    match args.get("update") {
        Some(update) => {
            let body = Some(update.clone());
            dispatch(app, caller, "PATCH", path, None, &body, None).await
        }
        None => dispatch(app, caller, "GET", path, None, &None, None).await,
    }
}

async fn transfer(
    app: &gproxy_app::AppHandle,
    caller: Option<&Parts>,
    args: &JsonObject,
) -> McpResult<CallToolResult> {
    match required_str(args, "action")? {
        "export" => {
            let body = Some(json!({
                "include_secrets": args
                    .get("include_secrets")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            }));
            dispatch(app, caller, "POST", "/admin/api/export", None, &body, None).await
        }
        "import" => {
            let export = args
                .get("export")
                .cloned()
                .ok_or_else(|| ErrorData::invalid_params("export is required", None))?;
            let body = Some(json!({
                "export": export,
                "source_master_key": str_arg(args, "source_master_key"),
            }));
            dispatch(app, caller, "POST", "/admin/api/import", None, &body, None).await
        }
        other => Err(ErrorData::invalid_params(
            format!("unknown transfer action `{other}`"),
            None,
        )),
    }
}

async fn diagnostics(
    app: &gproxy_app::AppHandle,
    caller: Option<&Parts>,
    args: &JsonObject,
) -> McpResult<CallToolResult> {
    let check = required_str(args, "check")?;
    // (method, path, body-building keys)
    let (method, path, body) = match check {
        "connectivity" => (
            "POST",
            "/admin/api/connectivity/test".to_owned(),
            Some(json!({
                "scope": str_arg(args, "scope").unwrap_or("global"),
                "provider_id": i64_arg(args, "provider_id"),
                "credential_id": i64_arg(args, "credential_id"),
                "proxy_url": str_arg(args, "proxy_url"),
            })),
        ),
        "model-test" => (
            "POST",
            "/admin/api/models/test".to_owned(),
            Some(json!({
                "provider_id": required_i64(args, "provider_id")?,
                "model_id": required_str(args, "model_id")?,
            })),
        ),
        "model-discover" => (
            "POST",
            "/admin/api/models/discover".to_owned(),
            Some(json!({ "provider_id": required_i64(args, "provider_id")? })),
        ),
        "channels" => ("GET", "/admin/api/channels".to_owned(), None),
        "tls-presets" => ("GET", "/admin/api/tls-presets".to_owned(), None),
        "rule-presets" => ("GET", "/admin/api/rule-presets".to_owned(), None),
        "quota-windows" => ("GET", "/admin/api/quota-windows".to_owned(), None),
        "credential-cycles" => ("GET", "/admin/api/credential-cycles".to_owned(), None),
        "credential-quota" => (
            "GET",
            format!(
                "/admin/api/credentials/{}/quota",
                required_i64(args, "credential_id")?
            ),
            None,
        ),
        "credential-quota-probe" => (
            "POST",
            format!(
                "/admin/api/credentials/{}/quota-probe",
                required_i64(args, "credential_id")?
            ),
            None,
        ),
        "credential-quota-reset" => (
            "POST",
            format!(
                "/admin/api/credentials/{}/quota-reset",
                required_i64(args, "credential_id")?
            ),
            None,
        ),
        "credential-health-reset" => (
            "POST",
            format!(
                "/admin/api/credentials/{}/health-reset",
                required_i64(args, "credential_id")?
            ),
            None,
        ),
        other => {
            return Err(ErrorData::invalid_params(
                format!("unknown diagnostic `{other}`"),
                None,
            ));
        }
    };
    dispatch(app, caller, method, &path, None, &body, None).await
}

/// Dispatch a synthetic admin API request and turn the response into a result.
#[allow(clippy::too_many_arguments)]
async fn dispatch(
    app: &gproxy_app::AppHandle,
    caller: Option<&Parts>,
    method: &str,
    path: &str,
    query: Option<Vec<(String, String)>>,
    body: &Option<Value>,
    headers: Option<&JsonObject>,
) -> McpResult<CallToolResult> {
    let (parts, body) = build_request(caller, method, path, query, body, headers)?;
    let Some(response) = app.admin_dispatch(&parts, body).await else {
        return Ok(CallToolResult::structured_error(json!({
            "error": format!("{method} {path} is not an admin API route"),
        })));
    };
    let status = response.status().as_u16();
    let bytes = response.into_body();
    let body = decode_body(&bytes);
    let summary = json!({
        "status": status,
        "ok": (200..300).contains(&status),
        "body": body,
    });
    if (200..300).contains(&status) {
        Ok(CallToolResult::structured(summary))
    } else {
        Ok(CallToolResult::structured_error(summary))
    }
}

/// Build the admin API request the tool will dispatch.
fn build_request(
    caller: Option<&Parts>,
    method: &str,
    path: &str,
    query: Option<Vec<(String, String)>>,
    body: &Option<Value>,
    headers: Option<&JsonObject>,
) -> McpResult<(Parts, Bytes)> {
    if !ALLOWED_METHODS.contains(&method) {
        return Err(ErrorData::invalid_params(
            format!("method `{method}` is not allowed"),
            None,
        ));
    }
    if !path.starts_with(ADMIN_PREFIX) {
        return Err(ErrorData::invalid_params(
            format!("path must start with {ADMIN_PREFIX}"),
            None,
        ));
    }
    if path == MCP_PATH {
        return Err(ErrorData::invalid_params(
            "refusing to call the MCP endpoint itself",
            None,
        ));
    }
    let method: Method = method
        .parse()
        .map_err(|_| ErrorData::invalid_params(format!("invalid method `{method}`"), None))?;
    let mut uri = path.parse::<Uri>().map_err(|error| {
        ErrorData::invalid_params(format!("invalid path `{path}`: {error}"), None)
    })?;
    if let Some(pairs) = query.as_ref().filter(|pairs| !pairs.is_empty()) {
        // Encode by hand rather than with `form_urlencoded`: that crate renders
        // a space as `+`, which the admin API's `form_urlencoded::parse` and
        // `serde_urlencoded` decode back to a space, silently corrupting values
        // that legitimately contain `+` (request ids, model names). Percent
        // encoding is unambiguous in both directions.
        let encoded = pairs
            .iter()
            .map(|(key, value)| format!("{}={}", encode_query_part(key), encode_query_part(value)))
            .collect::<Vec<_>>()
            .join("&");
        let path_and_query = format!("{}?{encoded}", uri.path());
        uri = path_and_query.parse::<Uri>().map_err(|error| {
            ErrorData::invalid_params(format!("invalid query for `{path}`: {error}"), None)
        })?;
    }
    let bytes = match body {
        Some(value) => Bytes::from(
            serde_json::to_vec(value)
                .map_err(|error| ErrorData::internal_error(error.to_string(), None))?,
        ),
        None => Bytes::new(),
    };
    let mut parts = match caller {
        Some(caller) => caller.clone(),
        None => Request::new(()).into_parts().0,
    };
    parts.method = method;
    parts.uri = uri;
    // The caller's framing describes the MCP envelope, not this payload.
    parts.headers.remove(CONTENT_LENGTH);
    parts.headers.remove(TRANSFER_ENCODING);
    if body.is_some() {
        parts
            .headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    }
    if let Some(headers) = headers {
        for (name, value) in headers {
            let (Ok(name), Some(value)) = (
                name.parse::<HeaderName>(),
                value
                    .as_str()
                    .and_then(|value| HeaderValue::from_str(value).ok()),
            ) else {
                return Err(ErrorData::invalid_params(
                    format!("invalid header `{name}`"),
                    None,
                ));
            };
            parts.headers.insert(name, value);
        }
    }
    Ok((parts, bytes))
}

/// Reduce a list response to the record with `id`.
fn filter_by_id(mut result: CallToolResult, id: i64) -> McpResult<CallToolResult> {
    let Some(value) = result.structured_content.take() else {
        return Ok(result);
    };
    let Some(records) = value.get("body").and_then(Value::as_array) else {
        // The list call failed, or the entity is not a list; pass it through.
        return Ok(result);
    };
    let found = records
        .iter()
        .find(|record| record.get("id").and_then(Value::as_i64) == Some(id))
        .cloned();
    match found {
        Some(record) => Ok(CallToolResult::structured(json!({
            "status": value.get("status").cloned().unwrap_or(Value::Null),
            "ok": true,
            "body": record,
        }))),
        None => Ok(CallToolResult::structured_error(json!({
            "status": 404,
            "ok": false,
            "error": format!("no record with id {id}"),
        }))),
    }
}

fn decode_body(bytes: &Bytes) -> Value {
    if bytes.is_empty() {
        return Value::Null;
    }
    serde_json::from_slice(bytes)
        .unwrap_or_else(|_| json!({ "raw": String::from_utf8_lossy(bytes) }))
}

/// Percent-encode one path segment.
fn encode_segment(value: &str) -> String {
    utf8_percent_encode(value, PATH_SEGMENT).to_string()
}

/// Percent-encode one query key or value.
fn encode_query_part(value: &str) -> String {
    utf8_percent_encode(value, QUERY_PART).to_string()
}

/// Build a query from the flat scalar keys present in `args`.
fn scalar_query(args: &JsonObject, keys: &[&str]) -> Option<Vec<(String, String)>> {
    let pairs = keys
        .iter()
        .filter_map(|key| {
            let value = args.get(*key)?;
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                Value::Bool(flag) => flag.to_string(),
                _ => return None,
            };
            Some(((*key).to_owned(), text))
        })
        .collect::<Vec<_>>();
    (!pairs.is_empty()).then_some(pairs)
}

fn str_arg<'a>(args: &'a JsonObject, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

/// Flatten a nested object of scalars into query pairs.
fn nested_scalar_query(args: &JsonObject, key: &str) -> Option<Vec<(String, String)>> {
    let object = args.get(key)?.as_object()?;
    let pairs = object
        .iter()
        .filter_map(|(name, value)| {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                Value::Bool(flag) => flag.to_string(),
                _ => return None,
            };
            Some((name.clone(), text))
        })
        .collect::<Vec<_>>();
    (!pairs.is_empty()).then_some(pairs)
}

/// Read a nested object of strings, used for extra request headers.
fn nested_string_map(args: &JsonObject, key: &str) -> Option<JsonObject> {
    let object = args.get(key)?.as_object()?;
    Some(
        object
            .iter()
            .filter_map(|(name, value)| {
                Some((name.clone(), Value::String(value.as_str()?.to_owned())))
            })
            .collect(),
    )
}

fn i64_arg(args: &JsonObject, key: &str) -> Option<i64> {
    match args.get(key)? {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn required_str<'a>(args: &'a JsonObject, key: &str) -> McpResult<&'a str> {
    str_arg(args, key)
        .ok_or_else(|| ErrorData::invalid_params(format!("`{key}` must be a string"), None))
}

fn required_i64(args: &JsonObject, key: &str) -> McpResult<i64> {
    i64_arg(args, key)
        .ok_or_else(|| ErrorData::invalid_params(format!("`{key}` must be an integer"), None))
}

fn entity(args: &JsonObject) -> McpResult<&str> {
    let entity = required_str(args, "entity")?;
    if catalog::is_entity(entity) {
        Ok(entity)
    } else {
        Err(ErrorData::invalid_params(
            format!("unknown entity `{entity}`"),
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        build_request, decode_body, filter_by_id, nested_scalar_query, nested_string_map, tools,
    };
    use bytes::Bytes;
    use http::request::Parts;
    use rmcp::model::CallToolResult;
    use serde_json::json;

    fn empty_parts() -> Parts {
        http::Request::new(()).into_parts().0
    }

    #[test]
    fn tool_names_are_unique_and_prefixed() {
        let tools = tools();
        let mut names = std::collections::HashSet::new();
        for tool in &tools {
            assert!(
                tool.name.starts_with("gproxy_"),
                "{} is unprefixed",
                tool.name
            );
            assert!(
                names.insert(tool.name.clone()),
                "duplicate tool {}",
                tool.name
            );
            assert!(tool.description.is_some());
        }
        assert_eq!(names.len(), tools.len());
    }

    #[test]
    fn every_tool_schema_is_an_object() {
        for tool in tools() {
            assert_eq!(
                tool.input_schema
                    .get("type")
                    .and_then(|value| value.as_str()),
                Some("object"),
                "{} has a non-object schema",
                tool.name
            );
        }
    }

    #[test]
    fn read_only_tools_carry_the_read_only_hint() {
        for tool in tools() {
            let read_only = matches!(
                tool.name.as_ref(),
                "gproxy_endpoints"
                    | "gproxy_openapi"
                    | "gproxy_list"
                    | "gproxy_get"
                    | "gproxy_audit"
                    | "gproxy_usage"
            );
            // `gproxy_create` only adds a record, so it is a write but not a
            // destructive one; every other non-read tool replaces or removes
            // stored state and says so.
            let destructive = !read_only && tool.name != "gproxy_create";
            let annotations = tool.annotations.clone().unwrap_or_default();
            assert_eq!(
                annotations.read_only_hint,
                Some(read_only),
                "{} has the wrong read-only hint",
                tool.name
            );
            assert_eq!(
                annotations.destructive_hint,
                Some(destructive),
                "{} has the wrong destructive hint",
                tool.name
            );
        }
    }

    #[test]
    fn build_request_rejects_paths_outside_the_admin_api() {
        let error = build_request(None, "GET", "/v1/models", None, &None, None).unwrap_err();
        assert!(error.message.contains("/admin/api/"));
    }

    #[test]
    fn build_request_refuses_the_mcp_endpoint() {
        let error = build_request(None, "POST", "/admin/api/mcp", None, &None, None).unwrap_err();
        assert!(error.message.contains("MCP endpoint"));
    }

    #[test]
    fn build_request_rejects_unknown_methods() {
        assert!(build_request(None, "PUT", "/admin/api/providers", None, &None, None).is_err());
        assert!(build_request(None, "TRACE", "/admin/api/providers", None, &None, None).is_err());
    }

    #[test]
    fn build_request_encodes_the_query_and_body() {
        let query = vec![("limit".to_owned(), "5".to_owned())];
        let body = Some(json!({ "name": "smoke" }));
        let (parts, bytes) = build_request(
            None,
            "POST",
            "/admin/api/providers",
            Some(query),
            &body,
            None,
        )
        .unwrap();
        assert_eq!(parts.method, http::Method::POST);
        assert_eq!(parts.uri.path(), "/admin/api/providers");
        assert_eq!(parts.uri.query(), Some("limit=5"));
        assert_eq!(
            parts.headers.get(http::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
        assert_eq!(bytes, Bytes::from_static(br#"{"name":"smoke"}"#));
    }

    #[test]
    fn build_request_percent_encodes_query_values() {
        let query = vec![
            ("request_id".to_owned(), "a+b c".to_owned()),
            ("model".to_owned(), "glm-5.3".to_owned()),
        ];
        let (parts, _) =
            build_request(None, "GET", "/admin/api/logs", Some(query), &None, None).unwrap();
        let query = parts.uri.query().unwrap();
        assert!(query.contains("a%2Bb%20c"), "{query} lost its `+` or space");
        assert!(!query.contains('+'), "{query} used `+` for a space");
        assert!(
            query.contains("model=glm-5.3"),
            "{query} over-encoded a model name"
        );
    }

    #[test]
    fn build_request_drops_the_caller_framing_headers() {
        let mut caller = empty_parts();
        caller
            .headers
            .insert("content-length", http::HeaderValue::from_static("42"));
        let (parts, bytes) = build_request(
            Some(&caller),
            "GET",
            "/admin/api/providers",
            None,
            &None,
            None,
        )
        .unwrap();
        assert!(parts.headers.get("content-length").is_none());
        assert!(bytes.is_empty());
    }

    #[test]
    fn build_request_keeps_the_caller_credentials() {
        let mut caller = empty_parts();
        caller.headers.insert(
            http::header::AUTHORIZATION,
            http::HeaderValue::from_static("Bearer sk-gp-test"),
        );
        caller
            .extensions
            .insert(gproxy_admin::AuthSource("203.0.113.7".into()));
        let (parts, _) = build_request(
            Some(&caller),
            "GET",
            "/admin/api/session",
            None,
            &None,
            None,
        )
        .unwrap();
        assert_eq!(
            parts.headers.get(http::header::AUTHORIZATION).unwrap(),
            "Bearer sk-gp-test"
        );
        assert_eq!(
            parts
                .extensions
                .get::<gproxy_admin::AuthSource>()
                .map(|source| source.0.as_str()),
            Some("203.0.113.7")
        );
    }

    #[test]
    fn build_request_applies_extra_headers() {
        let headers = json!({ "x-request-id": "abc" })
            .as_object()
            .cloned()
            .unwrap();
        let (parts, _) = build_request(
            None,
            "GET",
            "/admin/api/session",
            None,
            &None,
            Some(&headers),
        )
        .unwrap();
        assert_eq!(parts.headers.get("x-request-id").unwrap(), "abc");
    }

    #[test]
    fn nested_objects_flatten_into_query_pairs() {
        let args = json!({ "query": { "limit": 5, "name": "x", "flag": true, "skip": null } })
            .as_object()
            .cloned()
            .unwrap();
        let mut pairs = nested_scalar_query(&args, "query").unwrap();
        pairs.sort();
        assert_eq!(
            pairs,
            vec![
                ("flag".to_owned(), "true".to_owned()),
                ("limit".to_owned(), "5".to_owned()),
                ("name".to_owned(), "x".to_owned()),
            ]
        );
        assert!(nested_scalar_query(&args, "missing").is_none());
    }

    #[test]
    fn header_objects_keep_only_string_values() {
        let args = json!({ "headers": { "a": "1", "b": 2 } })
            .as_object()
            .cloned()
            .unwrap();
        let headers = nested_string_map(&args, "headers").unwrap();
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("a"), Some(&json!("1")));
    }

    #[test]
    fn decode_body_handles_json_text_and_emptiness() {
        assert!(decode_body(&Bytes::new()).is_null());
        assert_eq!(decode_body(&Bytes::from_static(b"[1,2]")), json!([1, 2]));
        assert_eq!(
            decode_body(&Bytes::from_static(b"not json")),
            json!({ "raw": "not json" })
        );
    }

    #[test]
    fn filter_by_id_narrows_a_list_response() {
        let listed = CallToolResult::structured(json!({
            "status": 200,
            "ok": true,
            "body": [{ "id": 1 }, { "id": 2 }],
        }));
        let narrowed = filter_by_id(listed, 2).unwrap();
        assert_eq!(
            narrowed.structured_content.unwrap()["body"],
            json!({ "id": 2 })
        );

        let listed = CallToolResult::structured(json!({ "status": 200, "body": [{ "id": 1 }] }));
        let missing = filter_by_id(listed, 9).unwrap();
        assert_eq!(missing.is_error, Some(true));
    }
}
