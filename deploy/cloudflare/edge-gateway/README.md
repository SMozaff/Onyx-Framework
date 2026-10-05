# ONYX Cloudflare Edge Gateway

This Worker is Tier 1 only. It is an ingress/proxy boundary in front of the
existing Rust/Axum ONYX API server. It does not authenticate users, authorize
commands, access PostgreSQL, or access object storage.

Set ONYX_ORIGIN_URL to the public HTTPS origin of the native ONYX API deployment.
The route/domain in wrangler.jsonc are placeholders and must be changed per deployment.

The native ONYX runtime remains responsible for JWT authentication and token
revocation, capability/command authorization, PostgreSQL transactions and
migrations, audit/security policy, and background jobs, outbox relay, scheduler,
and retry semantics.

WebSocket upgrades are intentionally preserved by the request proxy so the
existing Axum WebSocket routes remain the application authority.

Deployment requires a Cloudflare zone with a proxied DNS record for the route.
Use npm install followed by npx wrangler deploy from this directory.
