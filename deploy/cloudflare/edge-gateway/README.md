# ONYX Cloudflare Edge Gateway

This Worker is Tier 1 only. It is an ingress/proxy boundary in front of the
existing Rust/Axum ONYX API server. It does not authenticate users, authorize
commands, access PostgreSQL, or access object storage.

## Domain-free deployment

The Worker uses Cloudflare's free `workers.dev` endpoint. No custom domain or
Cloudflare DNS zone is required.

The repository Worker name is `onyx-framework`, matching the currently
deployed Cloudflare Worker.

The Worker requires `ONYX_ORIGIN_URL`, which must be the public HTTPS origin
of the native ONYX API. The Worker deliberately rejects non-HTTPS origins.

For development, a Cloudflare Quick Tunnel can provide a temporary
`trycloudflare.com` origin:

```text
cloudflared tunnel --url http://localhost:3000
```

Then use the generated HTTPS URL as `ONYX_ORIGIN_URL`. Quick Tunnels are
intended for development/testing and their hostname changes when the tunnel is
restarted.

For a stable production deployment, use a managed/public HTTPS origin for the
native ONYX API. A custom domain for the Worker itself is optional and can be
added later.

## Direct local deployment

From this directory:

```bash
npm install
npx wrangler deploy --var ONYX_ORIGIN_URL:https://<your-onyx-api-origin>
```

Wrangler will deploy to the Worker's `workers.dev` hostname.

## GitHub deployment

A manual GitHub Actions workflow is provided at
`.github/workflows/deploy-cloudflare-edge.yml`.

Configure these GitHub repository/environment secrets before running it:

- `CLOUDFLARE_ACCOUNT_ID`
- `CLOUDFLARE_API_TOKEN`
- `ONYX_ORIGIN_URL`

The Cloudflare API token must have permission to deploy Workers in the target
account. `ONYX_ORIGIN_URL` must be an HTTPS URL; do not commit it to source
control.

## Architecture boundary

The native ONYX runtime remains responsible for JWT authentication and token
revocation, capability/command authorization, PostgreSQL transactions and
migrations, audit/security policy, and background jobs, outbox relay, scheduler,
and retry semantics.

WebSocket upgrades are intentionally preserved by the request proxy so the
existing Axum WebSocket routes remain the application authority.

## Production DNS

A custom domain is not required for this architecture to work. If a domain is
later acquired, use a Cloudflare Worker Route in front of the existing ONYX
origin rather than moving the native Axum runtime into Workers.
