//! OpenAPI description of the admin API.
//!
//! The document is generated from [`crate::catalog`], the same table that backs
//! the `gproxy_endpoints` tool. Request and response bodies are described
//! permissively — the admin API validates bodies in Rust, and mirroring a few
//! hundred DTOs here would duplicate that code and inevitably drift — but the
//! route list, methods, path parameters and auth schemes are exact.

use serde_json::{Map, Value, json};

use crate::catalog::{self, Endpoint};

/// Build the OpenAPI 3.1 document.
pub(crate) fn document() -> Value {
    let mut paths: Map<String, Value> = Map::new();
    for endpoint in catalog::ENDPOINTS {
        let operation = operation(endpoint);
        let entry = paths
            .entry(endpoint.path.to_owned())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(entry) = entry.as_object_mut() {
            entry.insert(endpoint.method.to_lowercase(), operation);
        }
    }
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "gproxy admin API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": DESCRIPTION,
        },
        "servers": [{ "url": "/" }],
        "tags": tags(),
        "paths": Value::Object(paths),
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "description": "An admin API key, sent as `Authorization: Bearer sk-gp-…`.",
                },
                "adminCookie": {
                    "type": "apiKey",
                    "in": "cookie",
                    "name": "gproxy_admin_session",
                    "description": "Browser session cookie issued by POST /admin/api/login.",
                },
            },
        },
        "security": [{ "bearerAuth": [] }, { "adminCookie": [] }],
    })
}

const DESCRIPTION: &str = "\
The gproxy admin API is the control plane behind the operator console. Every \
route lives under /admin/api and speaks JSON.

Authentication is either an admin API key (`Authorization: Bearer sk-gp-…`) or \
the console's session cookie. Requests authenticated with a key are not subject \
to the same-origin check applied to cookie-authenticated writes, which is what \
lets non-browser clients drive the API.

Reads are safe to repeat. Writes take effect immediately: updates replace the \
whole record rather than merging, and deletes are irreversible.";

/// Tag list, derived from the entities so the document stays in step.
fn tags() -> Value {
    let mut seen: Vec<&str> = Vec::new();
    for endpoint in catalog::ENDPOINTS {
        if let Some(tag) = tag(endpoint)
            && !seen.contains(&tag)
        {
            seen.push(tag);
        }
    }
    Value::Array(
        seen.into_iter()
            .map(|tag| {
                json!({
                    "name": tag,
                    "description": catalog::entity_label(tag)
                        .map(|label| format!("{label} records."))
                        .unwrap_or_else(|| format!("{tag} routes.")),
                })
            })
            .collect(),
    )
}

/// The tag a route belongs to: its entity, else the first path segment.
fn tag(endpoint: &Endpoint) -> Option<&str> {
    if let Some(entity) = endpoint.entity {
        return Some(entity);
    }
    let rest = endpoint.path.strip_prefix("/admin/api/")?;
    Some(rest.split('/').next().unwrap_or(rest))
}

fn operation(endpoint: &Endpoint) -> Value {
    let mut operation = Map::new();
    // The catalog summary already reads as a sentence; the path is spelled out
    // in the object key, so no repetition is needed here.
    operation.insert(
        "summary".to_owned(),
        Value::String(endpoint.summary.to_owned()),
    );
    if let Some(tag) = tag(endpoint) {
        operation.insert("tags".to_owned(), json!([tag]));
    }
    let parameters = endpoint
        .parameters()
        .into_iter()
        .map(|name| {
            json!({
                "name": name,
                "in": "path",
                "required": true,
                "schema": {
                    "type": if name == "id" || name.ends_with("_id") { "integer" } else { "string" },
                },
            })
        })
        .collect::<Vec<_>>();
    if !parameters.is_empty() {
        operation.insert("parameters".to_owned(), Value::Array(parameters));
    }
    if let Some(schema) = request_body(endpoint) {
        operation.insert(
            "requestBody".to_owned(),
            json!({
                "required": true,
                "content": { "application/json": { "schema": schema } },
            }),
        );
    }
    operation.insert(
        "responses".to_owned(),
        json!({
            "200": {
                "description": "Success.",
                "content": { "application/json": { "schema": { "type": "object", "additionalProperties": true } } },
            },
            "201": { "description": "Created." },
            "400": error_response("The request was rejected; see the `error.message` field."),
            "401": error_response("Authentication is missing or invalid."),
            "403": error_response("The credential is valid but not permitted."),
            "404": error_response("No such route or record."),
        }),
    );
    Value::Object(operation)
}

/// A permissive body schema for the methods that carry one.
fn request_body(endpoint: &Endpoint) -> Option<Value> {
    match endpoint.method {
        "POST" | "PATCH" | "PUT" => Some(json!({ "type": "object", "additionalProperties": true })),
        _ => None,
    }
}

fn error_response(description: &str) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": {
                    "type": "object",
                    "properties": { "error": { "type": "object", "additionalProperties": true } },
                },
            },
        },
    })
}

#[cfg(test)]
mod tests {
    use super::document;
    use crate::catalog;

    #[test]
    fn document_is_openapi_31_with_paths() {
        let document = document();
        assert_eq!(document["openapi"], "3.1.0");
        assert_eq!(document["info"]["title"], "gproxy admin API");
        assert!(document["paths"].as_object().unwrap().len() > 20);
    }

    #[test]
    fn every_catalog_route_is_documented() {
        let document = document();
        let paths = document["paths"].as_object().unwrap();
        for endpoint in catalog::ENDPOINTS {
            let item = paths
                .get(endpoint.path)
                .unwrap_or_else(|| panic!("{} is undocumented", endpoint.path));
            assert!(
                item.get(endpoint.method.to_lowercase()).is_some(),
                "{} {} is undocumented",
                endpoint.method,
                endpoint.path
            );
        }
    }

    #[test]
    fn path_parameters_are_declared_and_named() {
        let document = document();
        let parameters = document["paths"]["/admin/api/providers/{id}"]["patch"]["parameters"]
            .as_array()
            .unwrap();
        assert_eq!(parameters.len(), 1);
        assert_eq!(parameters[0]["name"], "id");
        assert_eq!(parameters[0]["in"], "path");
        assert_eq!(parameters[0]["required"], true);
        assert_eq!(parameters[0]["schema"]["type"], "integer");
    }

    #[test]
    fn writes_carry_a_body_and_reads_do_not() {
        let document = document();
        assert!(document["paths"]["/admin/api/providers"]["post"]["requestBody"].is_object());
        assert!(document["paths"]["/admin/api/providers"]["get"]["requestBody"].is_null());
    }

    #[test]
    fn security_schemes_cover_key_and_cookie_auth() {
        let document = document();
        let schemes = &document["components"]["securitySchemes"];
        assert_eq!(schemes["bearerAuth"]["scheme"], "bearer");
        assert_eq!(schemes["adminCookie"]["name"], "gproxy_admin_session");
        assert_eq!(document["security"].as_array().unwrap().len(), 2);
    }
}
