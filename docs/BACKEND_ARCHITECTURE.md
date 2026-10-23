# ONYX Backend Architecture

## Current deployment model

ONYX currently has **two distinct backend implementations** in the repository. They are not one tiered request path and the Cloudflare Worker does not proxy to the native Axum API.

1. **Cloudflare Worker API** — `deploy/cloudflare/edge-gateway`. This is the supported remote API for Admin, Staff, web, and mobile clients. It implements its own route handlers, authentication and authorization, command/query handling, and persistence against Cloudflare D1. It uses Durable Objects for event/relay hubs. Its current contract and deployment behavior are described in that directory's README and Wrangler configuration.
2. **Native Rust API and worker** — `crates/bins/api-server` and `crates/bins/worker`. The Axum API and Rust background worker are a separate runtime using the Rust domain/application layers and configured persistence adapters, including SQLx-backed PostgreSQL/SQLite. The native API rejects the reserved `allfather` username; All-Father authentication is supported only through the Worker.

Do not describe the Worker as a transparent proxy, or assume that routes, storage, authorization logic, or operational state are shared automatically between the two implementations.

## Cloudflare Worker boundary

The Worker is authoritative for the remote clients that target it:

- **Authentication:** `POST /api/auth/login` accepts ONYX username/password credentials and issues ONYX access/refresh tokens. Password verification uses the Worker's `argon2-wasm-edge` implementation.
- **Authorization:** protected requests re-read the relevant active principal, organization, role/admin state, tenant binding, and revocation state from D1. Client-provided identity or role fields do not grant authority.
- **Persistence:** D1 stores Worker-managed principals, password hashes, organizations, roles, sessions/revocations, and application records handled by its route layer.
- **Realtime/relay:** Durable Objects back the Worker event and relay hubs.
- **Object storage:** optional Hugging Face S3-compatible configuration is supplied through server-side Worker bindings/environment configuration where used.

The Worker does not forward requests to the legacy Axum deployment. Its route coverage and persistence semantics must be validated against the actual Worker source and tests.

## Native Rust boundary

The native runtime is a separate implementation:

- `crates/bins/api-server` exposes Axum routes.
- `crates/bins/worker` runs native background processing.
- Rust domain and application crates hold native business logic and ports.
- Persistence adapters include SQLx-based PostgreSQL and SQLite implementations; other adapters are selected through their configured ports.
- Native authentication/session behavior and route contracts are implemented in the Rust API. They are not automatically equivalent to Worker/D1 behavior.

The native API must not be described as the authoritative All-Father login path. The current Rust login code reserves the `allfather` username for the Worker path.

## Data and deployment boundaries

The Worker/D1 runtime and native SQLx-backed runtime have separate persistence and deployment configurations. No shared database, transaction, replication, or cross-runtime authorization guarantee should be inferred unless a specific implementation and configuration establish it.

Likewise, a successful build or test of one backend does not establish route parity, data parity, deployment success, or end-to-end functionality in the other backend.

## Verification expectations

When changing an API contract, compare all of the following separately:

- Worker route handler and Worker tests
- Native Axum route handler and native tests
- Client base URL and request/response expectations
- Persistence and authorization semantics for the backend that the client actually calls
- Current deployment configuration and live deployment evidence, when making deployment claims

Report unverified parity or deployment state explicitly rather than inferring it from source structure.
