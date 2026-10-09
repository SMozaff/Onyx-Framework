# ONYX Cloudflare Worker migration

This directory is now the start of the ONYX backend migration to Cloudflare's free Workers platform. It no longer proxies requests to Render or any other Axum/container origin.

The target architecture is:

```text
Mobile / Admin / Staff
        |
        v
Cloudflare Worker (free workers.dev)
        |
        +---- D1 (free SQLite-compatible database)
        |
        +---- Clerk / existing identity boundary
        |
        +---- Hugging Face object storage for existing blob data
```

Cloudflare Workers Free is the runtime boundary. No custom domain is required: the `workers.dev` hostname is sufficient. The migration deliberately does not use Cloudflare Containers, Workers Paid, Render, or another paid/container-only service.

## Current state

Phase 1 has converted the Worker from an origin proxy into an origin-free application boundary:

- `/health` is implemented locally in the Worker.
- `/ready` checks the D1 binding when it is configured.
- There is no `ONYX_ORIGIN_URL` and no upstream `fetch()` to Render.
- A first D1 foundation migration mirrors the existing SQLite `users` contract.
- Unported ONYX API routes return `501 API_MIGRATION_IN_PROGRESS` rather than silently routing traffic to the unreachable legacy backend.

This is intentional. The Worker is not claimed to be a complete replacement until each API contract has been ported and verified.

## Bootstrap the free D1 database

From this directory, after authenticating Wrangler to the Cloudflare account:

```bash
npm install
npm run d1:create
```

Wrangler will print the D1 `database_id`. Add that ID to `wrangler.toml` by enabling the `[[d1_databases]]` block and replacing the placeholder.

For local development:

```bash
npm run d1:local
npm run dev
```

For the remote D1 database:

```bash
npm run d1:remote
npm run deploy
```

The Cloudflare account must be the user's existing account. The repository does not contain or require a Cloudflare API credential.

## Migration rule

The Rust/Axum API remains the reference implementation while individual contracts are ported. We do not rewrite the whole service blindly. For every route, the migration must preserve:

1. request and response JSON contracts;
2. authentication and capability checks;
3. tenant/organization isolation;
4. idempotency and audit semantics where applicable;
5. existing D1-compatible persistence behavior;
6. client compatibility for Mobile, Admin, and Staff shells;
7. automated tests before the route is switched from `501` to live behavior.

Only after all required routes are live and verified will the legacy Render deployment/configuration be removed.
