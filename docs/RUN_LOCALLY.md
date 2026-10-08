> **Development has moved to GitHub Codespaces** (`.devcontainer/`) — see the
> repo README's "Development environment" section. Everything below is kept
> for the specific cases a Codespace cannot cover (a physical phone on your
> own LAN for P2P/discovery testing, offline-from-GitHub work, etc.), not as
> the default path. It also predates this session's fixes to several build
> issues local machines hit that a Codespace does not (Google-endpoint
> geo-blocking, host RAM limits corrupting the Cargo build cache — see
> `MEMORY.md` if replicating any of this outside the container).

# ONYX — Run It Yourself (verified working build)

This is the fixed tree from audit fixes **C-01**, **C-02** and **H-01**. Everything
below was executed end-to-end in a clean sandbox before this document was written.

---

## 1. Prerequisites

* Rust **1.97.1** (pinned in `rust-toolchain.toml`)
* Python 3 (only to generate a random key below)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
  --default-toolchain 1.97.1 --profile minimal --component clippy,rustfmt
```

## 2. Build

```bash
export SQLX_OFFLINE=true          # the repo ships .sqlx offline metadata
cargo build -p api-server --bin api-server
```

> **`desktop-shell` is excluded on a headless box.** It is a Tauri crate and needs GTK:
> `apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev`.
> This is audit finding **M-06** — unrelated to the fixes here.

## 3. Run (SQLite, no external services needed)

`ONYX_AUTHORITY_SIGNING_KEY` **must** carry an encoding prefix (`hex:`, `base64:`
or `base64url:`) and decode to exactly 32 bytes. Without a prefix the raw string
bytes are used and startup fails with *"Ed25519 signing seed must contain exactly
32 bytes"*.

```bash
export DATABASE_URL="sqlite:/tmp/onyx.db?mode=rwc"
export ONYX_ENV=development
export ONYX_BIND=127.0.0.1:3000
export ONYX_METRICS_BIND=127.0.0.1:9090
export ONYX_BOOTSTRAP_TOKEN="demo-bootstrap-token-abc123"
export ONYX_AUTHORITY_SIGNING_KEY="hex:$(python3 -c 'import secrets;print(secrets.token_hex(32))')"

./target/debug/api-server
```

`curl -s localhost:3000/health` → `{"status":"ok"}`

---

## 4. Verification transcript (actual output)

### The old demo credentials are gone

```
POST /api/auth/login  {"username":"operator","password":"onyx"}
→ HTTP 401  INVALID_CREDENTIALS
```

### Bootstrap fails closed

| Case | Result |
|---|---|
| No `ONYX_BOOTSTRAP_TOKEN` set | `403 BOOTSTRAP_DISABLED` |
| Wrong token | `403 BOOTSTRAP_TOKEN_INVALID` |
| Password under 12 chars | `400 WEAK_PASSWORD` |
| Correct token, empty store | `201 Created` |
| **Correct token, second attempt** | **`409 BOOTSTRAP_ALREADY_COMPLETED`** |

The last row is the one-time guarantee: once any user exists the endpoint is
permanently closed, so a leaked token cannot add a back-door admin later.

```bash
curl -X POST localhost:3000/api/admin/bootstrap \
  -H 'Content-Type: application/json' \
  -H 'x-onyx-bootstrap-token: demo-bootstrap-token-abc123' \
  -d '{"username":"Admin","password":"correct horse battery staple"}'
```
```json
{"id":"8a09d988-...","username":"admin","organization_id":"11111111-...",
 "is_admin":true,"is_active":true}
```
Note `"Admin"` was stored as `"admin"` — usernames are lowercased and the unique
index is on `LOWER(username)`, so `Admin` and `admin` cannot both exist.

### Login issues a real Ed25519 JWT

```bash
curl -X POST localhost:3000/api/auth/login -H 'Content-Type: application/json' \
  -d '{"username":"ADMIN","password":"correct horse battery staple"}'
```
Returns `access_token` (1 h) + `refresh_token` (7 d). Decoded payload confirms the
**uniform scope was preserved exactly** as decided — `sub` and `organization_id`
now come from the user row instead of the deleted constants:

```json
{"sub":"8a09d988-...","username":"admin",
 "organization_id":"11111111-1111-1111-1111-111111111111",
 "token_type":"access",
 "scope":{"object_type":"*","object_id":null,
          "command_types":["notification.Acknowledge","approval.Approve","approval.Reject"],
          "delegation_depth":0}}
```

### What is actually stored

```
username   : admin
stored hash: $argon2id$v=19$m=19456,t=2,p=1$BhmN0xKjEMxiBg1mqmXclg$rffiWKf205...
```

PHC format, Argon2id, m=19456 KiB / t=2 / p=1 (OWASP), unique salt per user.
Two users with the same password produce different hashes.

### Authorization and lifecycle

| Case | Result |
|---|---|
| `GET /api/admin/users` with no token | `401` |
| Same, as non-admin `alice` | `403 ADMIN_REQUIRED` |
| Same, as `admin` | `200` + user list (**no `password_hash` field**) |
| Deactivate `alice`, then alice logs in | `401 INVALID_CREDENTIALS` |
| Admin deactivates **self** | `400 CANNOT_DEACTIVATE_SELF` |

Wrong-password, unknown-user and disabled-user all return the **identical**
`401 INVALID_CREDENTIALS` body, and the unknown/disabled paths run a dummy Argon2id
verification so response latency does not reveal which usernames exist.

---

## 5. Endpoint reference

| Method | Path | Auth |
|---|---|---|
| POST | `/api/auth/login` | none |
| POST | `/api/auth/logout` | Bearer |
| POST | `/api/admin/bootstrap` | `x-onyx-bootstrap-token`, empty store only |
| POST | `/api/admin/users` | Bearer + admin |
| GET | `/api/admin/users` | Bearer + admin |
| POST | `/api/admin/users/:id/deactivate` | Bearer + admin |
| POST | `/api/admin/users/:id/activate` | Bearer + admin |
| POST | `/api/admin/users/:id/password` | Bearer + admin |

## 6. Production deployment note

Set `ONYX_BOOTSTRAP_TOKEN`, call `/api/admin/bootstrap` once, then **remove the
variable**. The server emits a `WARN` telling you to do exactly that. Leaving it
set is harmless once a user exists (the endpoint is already closed) but removing
it eliminates the credential entirely.

## 7. Known-broken things NOT addressed yet

Updated status of these audit findings — most have since been resolved by the
H1-H7 hardening pass documented in `DECISIONS.md`; only H-04 is still open:

* ~~**Callers still using `operator`/`onyx`**~~ — resolved (audit fix H-01):
  `tests/end-to-end/approval_workflow.rs`, `web-ui/tests/mocks/server.ts`,
  and the Login page's prefilled username all bootstrap a real user instead.
  `tests/integration/log_redaction_tests.rs`'s `"operator"` string was never a
  login call in the first place -- it's an arbitrary field value in a
  log-redaction unit test.
* ~~**H-02** token revocation was an in-memory `HashSet`~~ -- resolved.
  Replaced with a shared, durable `TokenRevocationStore` (Postgres-backed,
  in-memory only as a pure-SQLite single-instance dev fallback); a revocation
  on one replica is now visible to every other replica reading the same
  store.
* ~~**H-03** CORS was `allow_origin(Any)`~~ -- resolved. Replaced with an
  explicit, env-driven allow-list (`ONYX_CORS_ALLOWED_ORIGINS`), required in
  production.
* **H-04** `sqlx 0.7.4` and two coexisting `rustls` versions (`0.21.12` and
  `0.23.43`, per `Cargo.lock`) still need upgrading -- not yet addressed.
