---
title: "Admin API & MCP"
description: "The /admin/api control plane, its authentication model, and the in-process MCP server that exposes it as tools for AI agents"
---

Everything the operator console does, it does by calling `/admin/api/**`.
That JSON surface is the whole control plane: the console is a thin client,
and the same routes can be driven from scripts, CI, curl, or the built-in
**MCP server**.

## Authentication

Both the browser and the mobile-less client paths land on the same
authentication code, and a request is accepted when either credential is
present.

| Credential | Header | Notes |
| --- | --- | --- |
| User API key | `Authorization: Bearer sk-gp-…` | The key must belong to a user with `is_admin`. Key authentication **skips the same-origin check**, so it can drive writes from a non-browser client. |
| Browser session | `Cookie: gproxy_admin_session=…` | Issued by `POST /admin/api/login`, valid for 12 hours. Writes additionally require `Origin` to match `Host`. |

`GET /admin/api/session` reports the setup state and is reachable without
credentials; `POST /admin/api/setup` performs first-boot initialization.

Because a key carries the privileges of its owner, a dedicated agent account
is the recommended pattern: create a user, mark it `is_admin`, mint one key
for the agent, and revoke that key to cut the agent off. Audit entries then
name a distinct subject rather than sharing the human operator's identity.

## Route shape

The API is CRUD-shaped and follows kebab-case paths.

| Operation | Route |
| --- | --- |
| List | `GET /admin/api/{entity}` |
| Create | `POST /admin/api/{entity}` → 201 |
| Update | `PATCH /admin/api/{entity}/{id}` |
| Delete | `DELETE /admin/api/{entity}/{id}` |
| Batch | `POST /admin/api/batch/{entity}` with `{"action": "enable" \| "disable" \| "delete", "ids": [...]}` |

`PATCH` replaces the stored record: send the **full** object, not a partial
patch. Read the record first, change the fields you mean to change, send it
back.

Alongside the generic CRUD routes sit the singletons and actions: usage
reporting, request logs, audit, instance/log/portal settings, configuration
export and import, connectivity and model tests, quota probes, and the
channel login flows. `GET /admin/api/channels`, `/tls-presets` and
`/rule-presets` describe what the running build supports.

`/admin/api/batch/{entity}` answers with a per-id outcome, so a partially
successful batch is visible instead of silent.

### Not part of the control plane

`/admin/api/native/**` (self-update and autostart) is handled by the host
binary before the admin dispatcher runs. It is **not** exposed through MCP —
an agent cannot restart or replace the running binary through these tools.

## MCP server

The gateway embeds a [Model Context Protocol](https://modelcontextprotocol.io)
server at:

```
POST /admin/api/mcp
```

It speaks the streamable HTTP transport and requires the same admin
credentials as the API it wraps. `GET /admin/api/openapi.json` serves the
generated OpenAPI 3.1 description of every route in the catalog.

Tool calls are dispatched **in-process** through the admin API: the caller's
credentials are replayed, and the call passes through the same authentication,
authorization, validation, audit and quota bookkeeping as a direct API request.
There is no second control path to keep in sync.

### Tools

| Tool | Purpose |
| --- | --- |
| `gproxy_endpoints` | List callable routes, filterable by entity, method or search term. |
| `gproxy_openapi` | Fetch the OpenAPI document, or one route's definition. |
| `gproxy_request` | Escape hatch: call any admin route directly (`method`, `path`, `query`, `body`, `headers`). |
| `gproxy_list` / `gproxy_get` | Read an entity, or one record. |
| `gproxy_create` / `gproxy_update` / `gproxy_delete` | Write one record. |
| `gproxy_batch` | Apply `enable` / `disable` / `delete` to many ids. |
| `gproxy_usage` | Aggregate, page, summarise or trend traffic usage. |
| `gproxy_logs` | List request logs, fetch one exchange, read or update capture settings. |
| `gproxy_audit` | Read the audit trail. |
| `gproxy_settings` | Read or update instance, log and portal settings. |
| `gproxy_transfer` | Export or import the whole configuration. |
| `gproxy_diagnostics` | Run connectivity / model tests, read channel, preset, quota and cycle catalogs. |

`gproxy_request` guarantees complete coverage: any route in the catalog is
reachable even when no dedicated tool fits. `/admin/api/mcp` itself is refused
as a target.

Tools carry MCP annotations so a client can warn before acting. `GET`-shaped
tools are `readOnlyHint`; writers that replace or remove stored state are
`destructiveHint`. `gproxy_create` is a write but additive, so it is neither.

### Configuration

| Variable | Default | Meaning |
| --- | --- | --- |
| `GPROXY_MCP_ALLOWED_HOSTS` | unset | Comma-separated allow list for the transport's DNS-rebinding check. Unset accepts any `Host`; set it when the endpoint is reachable without a trusted reverse proxy. |

Request bodies are capped at 32 MiB so a configuration import fits, and SSE
keep-alive frames are sent every 30 seconds.

### Connecting a client

Point any streamable-HTTP MCP client at the endpoint and supply the admin key
as a bearer token. For example, with `pi-mcp-adapter`:

```json
{
  "mcpServers": {
    "gproxy": {
      "url": "https://ai.debin.cc/admin/api/mcp",
      "bearerToken": "sk-gp-…"
    }
  }
}
```

Over the loopback interface the same endpoint is
`http://127.0.0.1:58881/admin/api/mcp`.