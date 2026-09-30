---
title: Portal & Web Surface
description: "The single web surface the gproxy binary serves, the portal sections, and how it is built and embedded"
---

The `gproxy` binary serves one React application, the **user portal**, at
`/portal` and its sub-paths. The build is embedded into the binary, so there
is nothing else to deploy.

| Path | Surface | API | Audience |
| --- | --- | --- | --- |
| `/portal` and `/portal/*` | User portal | `/portal/api/**` | users with a password |
| `/` | Redirects to `/portal` | none | anyone reaching the port |
| `/admin/*` (HTML) | Removed | none | redirects to `/portal` |
| `/admin/api/**` | Admin API | `/admin/api/**` | administrators, scripts, MCP clients |

Everything else on the port is gateway traffic authenticated by API key
(see [Routing & Endpoints](/reference/routing-table/)).

> **This fork has no operator console.** The admin API and its in-process
> [MCP server](/reference/admin-api/) replace it, so the binary ships only the
> portal's HTML. `/admin` and `/admin/*` requests that are not API calls
> answer `302` to `/portal` instead of a 404, because those URLs are already
> in people's bookmarks. The admin API keeps working exactly as before.

## First Boot

`GET /admin/api/session` reports `setup_required: true` until an
administrator exists. `POST /admin/api/setup` accepts one username and
password, creates the first admin, opens a session and records an
`auth.setup` audit event. The route is rate limited to four attempts per
minute per source address.

To skip the API call, start the binary with `GPROXY_ADMIN_PASSWORD` (and
optionally `GPROXY_ADMIN_USER`, default `admin`). The account is created on
first run, an admin API key is generated or taken from
`GPROXY_BOOTSTRAP_ADMIN_API_KEY`, and `GPROXY_BOOTSTRAP_CHANNELS` can create
empty providers for the listed channel ids. The bootstrap key and channels
apply only on first run, but the named administrator's password is
reapplied on every start, so remove `GPROXY_ADMIN_PASSWORD` once you have
logged in. See [Configuration](/reference/configuration/).

## Administrator Credentials

The portal signs in users with a password. Administrators reach the API in
one of two ways:

| Credential | Header | Notes |
| --- | --- | --- |
| User API key | `Authorization: Bearer sk-gp-…` | The key must belong to a user with `is_admin`. Bearer calls skip the same-origin check, so scripts and MCP clients can drive writes. |
| Browser session | `Cookie: gproxy_admin_session=…` | Issued by `POST /admin/api/login`, valid for 12 hours. Writes additionally require `Origin` to match `Host`. |

`POST /admin/api/login` and `/logout` are audited as `auth.login` and
`auth.logout`. See [Admin API & MCP](/reference/admin-api/) for the route
catalogue and the tool surface.

## The Portal

Any user with a password can sign in at `/portal`
(`POST /portal/api/login`; cookie `gproxy_portal_session`, 12 hours).
Administrators create users and set their initial password through the admin
API; users change it in the portal.

The sidebar links five sections. Each is a real, bookmarkable URL.

| Section | Path | What it does |
| --- | --- | --- |
| Overview | `/portal` | Spending quota windows applied to the account, and the latest settled requests when the administrator enables them. |
| Connect | `/portal/connect` | Pick an allowed model and copy a ready-to-run snippet: curl, OpenAI Python, Claude Python, Gemini Python, Codex CLI config, Claude Code environment. Snippets are limited to the wire formats the model can serve. The allowed-model catalog is listed below the snippets. |
| Usage | `/portal/usage` | Settled requests, input, output and cached tokens, and cost over 1, 7 or 30 days, next to the quota windows. |
| API keys | `/portal/keys` | Create keys with prefix `sk` (API clients) or `at` (Codex access-token login) and an optional label; the key is shown once. List, reveal and revoke your own keys. |
| Authorized sessions | `/portal/sessions` | Applications you signed in to with OAuth, with first login, last refresh and expiry. Revoking one stops its access and refresh tokens. |

Keys have the form `<prefix>-gp-<random>`. The Codex and Claude Code
snippets are explained in [CLI Clients](/guides/cli-clients/).

### Settings

The sidebar footer shows the signed-in user. Selecting it opens a menu with
**Settings**, which slides open a drawer holding the password form and the
theme/language controls, and **Sign out**. Theme and language are stored in
the browser (`gproxy-console-theme`, `gproxy-console-lang`); the sidebar's
own width and collapsed state are stored under
`gproxy.portal.sidebar.preferences`.

### Keyboard and Small Screens

Table rows and cards that open a detail are focusable and respond to Enter
and Space. The sidebar resize handle accepts the arrow keys, Home and End,
and remembers its width. Below the `lg` breakpoint the sidebar becomes a
horizontal, scrollable bar.

## Portal Setting

The portal's one setting, whether users may see recent settled requests,
lives in `GET` and `PATCH /admin/api/portal-settings`.

## Building and Embedding the Portal

The web application lives in `console/` and is managed with pnpm.

```bash
cd console
pnpm install
pnpm build      # tsc -b, vite build, then scripts/sync-to-embed.mjs
```

The last step copies `console/dist/` into
`crates/gproxy-host-axum/assets/web/`, which `rust-embed` compiles into the
binary. Rebuild `gproxy` afterwards. A binary built without that directory
still serves the API; page requests answer
`web assets are not embedded; run pnpm build in console/ and rebuild gproxy`.

For development, `pnpm dev` starts Vite and proxies the admin and portal
APIs to a running backend. Finish console changes with `pnpm lint` and
`pnpm test`. Types under `console/src/generated/` are produced from the Rust
DTOs by `ts-rs` during `cargo test` and are never edited by hand; the export
list in `crates/gproxy-admin/src/dto/export.rs` is trimmed to the portal's
DTOs so the generator does not emit unused files.

The embedded `index.html` is served for `/`, `/portal` and any
`/portal/<section>` deep link. `/portal/api/**` is dispatched before the
static handler and never falls through to the shell. Hashed files under
`/assets/` are cached for a year, the HTML is `no-cache`, and
`/build-info.js` injects the build identity.