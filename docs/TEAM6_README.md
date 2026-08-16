# ONYX Team 6 — Integrated Web Thin Client

This handoff implements the frozen Team 6 scope:

- React/TypeScript/Vite SPA in `web-ui/`
- Axum HTTP/WebSocket routes in `crates/bins/api-server/src/routes/`
- Frozen OpenAPI 3.0 contract in `docs/api/openapi.json`
- Deterministic SQLite seed in `tests/fixtures/seed.sql`
- Binding rulings and implementation deviations in `DECISIONS.md`

## Architectural boundary

The browser is a stateless thin client. It contains no Rust core, operation log, CRDT engine, domain state machine, offline command queue, optimistic domain mutation, file-management flow, blueprint authoring, meeting chat, or synchronization participation.

## Local integrated run

```bash
# Terminal 1 — backend
cargo run --package api-server

# Terminal 2 — web client
cd web-ui
npm install
npm run dev
```

Default addresses:

- API: `http://127.0.0.1:3000`
- WebSocket: `ws://127.0.0.1:3000/api/events`
- Vite UI: `http://127.0.0.1:5173`

Deterministic development credentials:

```text
username: operator
password: onyx
organization_id: 11111111-1111-1111-1111-111111111111
```

Override backend configuration with:

```bash
DATABASE_URL='sqlite://onyx-team6.db?mode=rwc'
ONYX_BIND='127.0.0.1:3000'
ONYX_JWT_SECRET='replace-this-secret'
```

## Quality gates

```bash
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Web
cd web-ui
npm run type-check
npm run lint
npm run test
npm run test:a11y
npm run feature-audit
npm run build        # includes the <500 KB gzip bundle gate
```

## Real-server E2E

The deterministic 429/500 paths are enabled only in test mode.

```bash
# Terminal 1
ONYX_TEST_MODE=1 cargo run --package api-server

# Terminal 2
cd web-ui
ONYX_E2E_BASE='http://127.0.0.1:3000' npm run test:e2e
```

The seven real-server journeys cover authentication, Mission/Task/Notification/Approval queries, the 401/403/409/429/500 error matrix, and logout.

## OpenAPI validation

```bash
npx @apidevtools/swagger-cli validate docs/api/openapi.json
```

`GET /api/query` uses `?envelope={base64url(JSON)}`. WebSocket authentication uses `?token={access_token}` as required by ruling T6-R6; default request tracing is intentionally disabled to prevent token leakage through URI logging.
