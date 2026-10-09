# ONYX Cloudflare Container API

Phase 1 moves the existing Rust/Axum ONYX API into a Cloudflare Container and
uses a Cloudflare Worker as its public ingress. The application code remains
the existing `api-server` binary; this is a hosting migration, not an API
rewrite.

## Architecture

```text
Mobile / Admin / Staff
          |
          v
Cloudflare Worker (`onyx-framework`)
          |
          v
Cloudflare Container (`OnyxApiContainer`)
          |
          v
Rust/Axum `api-server` :10000
          |
          +--> Clerk
          +--> PostgreSQL
          +--> Hugging Face S3 blob storage
```

The Worker routes requests directly to the singleton `production` Container.
The Container class preserves WebSocket forwarding by using the Container
`fetch()` path rather than `containerFetch()`.

## Container image

Wrangler builds the repository-root `Dockerfile` using the repository root as
the Docker build context. The existing production image remains the source of
the Axum API; no Render-specific runtime behavior is required.

The API listens on `0.0.0.0:10000`. Cloudflare's Container binding uses port
10000 as its default port and checks `/ready` during startup.

## Runtime configuration

The Worker passes the ONYX production runtime configuration into the Container.
Sensitive values are Cloudflare Worker secrets and are never stored in this
repository.

Required Cloudflare secrets:

- `DATABASE_URL`
- `ONYX_GOVERNANCE_DATABASE_URL`
- `ONYX_AUTHORITY_SIGNING_KEY`
- `ONYX_CORS_ALLOWED_ORIGINS`
- `CLERK_ISSUER`
- `CLERK_SECRET_KEY`
- `CLERK_JWKS_URL`
- `ONYX_BLOB_STORE_S3_ACCESS_KEY_ID`
- `ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY`

The non-secret blob configuration remains:

- `ONYX_BLOB_STORE_BACKEND=huggingface`
- `ONYX_BLOB_STORE_S3_ENDPOINT=https://s3.hf.co/Arronthemalkavian`
- `ONYX_BLOB_STORE_S3_BUCKET=onyx`

The GitHub Actions deployment workflow expects the corresponding GitHub
repository secrets. It validates them and syncs them to Cloudflare with
`wrangler secret put` before deploying the Worker and Container.

## Deployment

Cloudflare Containers are available on the Workers Paid plan. From this
directory, with Docker available locally and a Cloudflare API token configured:

```bash
npm install
npx wrangler deploy
```

For GitHub Actions, run the `Deploy ONYX Cloudflare Container` workflow after
configuring the Cloudflare account/API-token secrets and the ONYX runtime
secrets listed above.

The Worker is configured for its `workers.dev` hostname. A custom domain can
be attached later without changing the Axum application.

## Phase 1 boundary

This phase deliberately does not rewrite Axum, move PostgreSQL, replace Clerk,
or replace the working Hugging Face blob store. It establishes Cloudflare as
the compute and ingress layer first so the application can be verified before
those independent migrations are considered.
