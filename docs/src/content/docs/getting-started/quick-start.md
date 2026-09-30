---
title: Quick Start
description: "From a downloaded binary to a working gateway: start gproxy, create the administrator, add a provider and a route, issue a key, and send a request."
---

This page takes a fresh native installation to its first successful request.
It assumes a portable archive from the [Downloads](/getting-started/downloads/)
page. An installer performs steps 1 and 2 for you and opens the portal.

## 1. Start gproxy

```bash
chmod +x ./gproxy
./gproxy
```

The server listens on `127.0.0.1:8787`, creates `./data/gproxy.db`, and logs
`GPROXY listening`. `gproxy --help` lists every flag. Each flag has a
`GPROXY_*` environment twin, and both can be written into a `.env` file in the
working directory or in the data directory. Precedence is flag, then
environment, then `./.env`, then `<data-dir>/.env`, then the default.

A minimal `.env`:

```env
GPROXY_HOST=127.0.0.1
GPROXY_PORT=8787
GPROXY_DATA_DIR=./data
GPROXY_MASTER_KEY=<standard base64, 32 bytes>
```

Generate the key with `openssl rand -base64 32`. Without it, credentials and
user keys are stored in plaintext. Set it before adding the first credential;
changing it afterwards is a rotation step described in
[Configuration](/reference/configuration/). Installers write a `.env` with a
generated key for you.

For a container:

```bash
docker run -d --name gproxy -p 8787:8787 \
  -v gproxy-data:/app/data \
  ghcr.io/leenhawk/gproxy:<tag>
```

## 2. Create the Administrator

A fresh store reports `setup_required: true` from
`GET /admin/api/session`. Create the administrator with
`POST /admin/api/setup` and a username and password; it opens a session and
signs you in. The admin API is then the control plane for everything below;
the user portal stays at `/portal`.

The administrator can also be created from `GPROXY_ADMIN_PASSWORD`; see
[Installation](/getting-started/installation/#first-boot).

## 3. Add a Provider and a Credential

Create a provider with `POST /admin/api/providers`: an upstream-facing name
(the stable identifier of this provider, also usable as a named-mode path
prefix), the channel, and the credential strategy — `round_robin` rotates
requests across the pool, `sticky` keeps each client key on one credential.
The channel decides which settings apply, for example a `base_url` for `custom`
or a `region` for `aws-bedrock`. Saving the provider seeds the channel's routing
rules and creates an empty private rule set named `<provider> · defaults`.

Then create a credential with `POST /admin/api/credentials`, passing the
provider's `id`. There are two ways to supply the secret:

- **Paste it.** Set `kind` to `api_key`, `oauth`, or `cookie` and fill the
  fields the channel declares, or put the raw credential object in `secret`.
  The label is optional; a default is derived from the secret.
- **Sign in.** Channels that declare a sign-in method use the login routes:
  `POST /admin/api/login/authcode/start` and `/authcode/complete` for browser
  sign-in (authorization code with PKCE), `/device/start` and `/device/poll`
  for device code, and `/cookie` for a pasted browser cookie. `codex` offers
  browser sign-in and device code; `claudecode` offers browser sign-in and
  browser cookie. Start the sign-in, approve it in the browser, then complete it
  with the callback URL or the device code. The tokens are stored sealed and
  refreshed by GPROXY under an exclusive lease.

Each credential carries a traffic weight, optional requests-per-minute and
tokens-per-minute limits, a proxy override, and its observed health.

Optionally ask the provider for the model ids it serves with
`POST /admin/api/models/discover`, then record them as `provider-models` rows
(`POST /admin/api/provider-models`) together with capabilities and default
prices.

## 4. Create a Route

Create a route with `POST /admin/api/routes`: a name and the maximum number of
attempts (the first attempt plus failovers). Then add members with
`POST /admin/api/route-members`: the `provider_id`, the `upstream_model` id, an
optional pinned credential, and the failover tier and weight. Tier 0 is
exhausted before tier 1 receives traffic; weight splits traffic among healthy
members in the same tier. Add members from other providers for failover.

Creating a route does not expose it yet. Create a model alias
(`POST /admin/api/model-aliases`) binding a public model name to the route;
that name is what clients send as `model`. Aggregated resolution runs alias,
then variant suffix, then public model name, then the route's members. A route
name on its own is reachable only through the named prefix, `/{route}/v1/...`.

## 5. Create a User and an API Key

Create a user with `POST /admin/api/users`. A password is optional; it is
needed only if the user should sign in to the portal. Then create a key with
`POST /admin/api/user-keys`, passing the `user_id`: a label, the prefix — `sk`
for API clients, `at` for Codex CLI access-token login — and an optional
`expires_at`. Copy the key when it is shown. The list shows only the prefix
afterwards; `POST /admin/api/user-keys/<id>/reveal` returns the full key as a
separate, audited action.

Permissions are default-deny. Create one with `POST /admin/api/permissions`:
affect (allow or deny), all providers or one provider, and all operations or
one operation group. It can be attached to the key, the user, a team, or an
organization and is inherited downward. Without an allow permission every
request from the key is refused with `403`. Rate limits and cost quotas are
created through their own routes.

## 6. Send a Request

Replace the placeholders with the key and the public model name:

```bash
curl http://127.0.0.1:8787/v1/chat/completions \
  -H "Authorization: Bearer sk-<your-key>" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "<public-model-name>",
    "messages": [
      { "role": "user", "content": "Say hello in one short sentence." }
    ]
  }'
```

The response carries an `x-request-id` header. `GET /admin/api/logs` lists the
request and `GET /admin/api/logs/<request_id>` returns the upstream call it
produced.

## Next Steps

- [First Request](/getting-started/first-request/) shows the same call in
  every accepted format, streaming, model listing, and the named prefix.
- Users with a password can sign in at `/portal` to create their own keys and
  copy connection snippets for curl, the OpenAI, Claude, and Gemini SDKs,
  Codex CLI, and Claude Code. See
  [Portal & Web Surface](/guides/console/).
- [CLI Clients](/guides/cli-clients/) covers pointing Codex CLI and Claude
  Code at the gateway.
