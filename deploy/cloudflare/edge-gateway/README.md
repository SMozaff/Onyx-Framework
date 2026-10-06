# ONYX Cloudflare Edge Gateway

This Worker is Tier 1 only. It is an ingress/proxy boundary in front of the
existing Rust/Axum ONYX API server. It does not authenticate users, authorize
commands, access PostgreSQL, or access object storage.

## Domain-free deployment

The Worker uses Cloudflare's free `workers.dev` endpoint. No custom domain or
Cloudflare DNS zone is required.

Set `ONYX_ORIGIN_URL` to the public HTTPS origin of the native ONYX API when
deploying. For development, a Cloudflare Quick Tunnel can provide a temporary
`trycloudflare.com` origin:

```text
cloudflared tunnel --url http://localhost:3000
```

Then configure the Worker with the generated HTTPS origin URL.

Quick Tunnels are intended for development/testing and their hostname changes
when the tunnel is restarted. For a stable production origin, use a managed
Cloudflare Tunnel or another public HTTPS deployment.

The native ONYX runtime remains responsible for JWT authentication and token
revocation, capability/command authorization, PostgreSQL transactions and
migrations, audit/security policy, and background jobs, outbox relay, scheduler,
and retry semantics.

WebSocket upgrades are intentionally preserved by the request proxy so the
existing Axum WebSocket routes remain the application authority.

Deployment:

```text
npm install
npx wrangler deploy
```

The Worker will receive a `workers.dev` hostname. Configure `ONYX_ORIGIN_URL`
as a Wrangler variable/environment value for that deployment.
