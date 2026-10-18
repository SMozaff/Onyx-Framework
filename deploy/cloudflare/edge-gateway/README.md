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
        +---- ONYX username/password authentication (Argon2id hashes in D1)
        |
        +---- Hugging Face object storage for existing blob data
```

Cloudflare Workers Free is the runtime boundary. No custom domain is required: the `workers.dev` hostname is sufficient. The migration deliberately does not use Cloudflare Containers, Workers Paid, Render, or another paid/container-only service.

## Locked authentication and authorization model

This model is binding for the migration:

- **All-Father:** ONYX username + password verified against the Argon2id hash stored on the canonical D1 principal. The account and its organization must be active, with explicit `ALL_FATHER` role and `is_admin=1`.
- **Organization Admin:** ONYX username or email plus password. Credentials are verified against the Argon2id password hash stored in D1.
- **Staff:** ONYX username or email plus password. Accounts are created, deactivated, and managed by an authorized organization Admin; staff cannot self-register or elevate roles.
- **D1 is authoritative** for ONYX users, password hashes, account status, roles, organization membership, and application authorization.
- **All-Father provisions organizations and Admins. Admins manage Staff only within their own organization.** The Worker must enforce these rules server-side.

Authentication endpoints:

- `POST /api/auth/login`: verify an ONYX username/email and password against D1, then issue an ONYX session.
- `POST /api/auth/refresh`: rotate a valid ONYX refresh token.
- `POST /api/auth/logout`: revoke supplied ONYX tokens.

All-Father and other user classes authenticate through `POST /api/auth/login`. The Worker verifies the D1 Argon2id hash, issues the existing ONYX token pair, and revalidates role, active status, tenant, organization, and revocation state on protected requests.

The D1 migrations own the canonical principal and role. The historical `0003_supabase_identity.sql` migration and `users.supabase_user_id` column remain only for migration-ledger compatibility; current runtime authentication does not read or write them. Admin, Staff, and All-Father passwords are Argon2id hashes in D1.

## Trust boundary: external claims vs ONYX-assigned privileges

Which facts are proven by an external identity provider, and which are assigned by ONYX. Only the
left column is ever *authenticated* externally; every privilege lives in the right column and is
decided server-side by the Worker against D1.

| Fact / claim | How it becomes trusted |
|---|---|
| Supabase JWT `sub`, `iss`, `exp`, `aud`, `nbf`, signature | Verified by the Worker (`src/supabase.ts`): JWKS signature, RS256/ES256 allowlist, exact issuer, expiry, audience (`authenticated`), and not-before when present |
| Supabase Auth user id, primary email, `email_confirmed_at` | Resolved server-side through `GET /auth/v1/user` with the presented bearer token; must equal the verified JWT `sub` and the configured All-Father email |
| `users.supabase_user_id` (external subject mapping) | **Owner-controlled provisioning only.** Login never creates, updates, or replaces it; an unmapped or mismatched subject is denied with `ALLFATHER_NOT_PROVISIONED` and no database write |
| `users.role` (`ALL_FATHER` / `ORGANIZATION_ADMIN` / `STAFF`) | Assigned by ONYX provisioning in D1 (All-Father provisions organizations and Admins; Admins manage Staff in their own organization) |
| `users.is_active`, `users.organization_id` | ONYX account lifecycle in D1; revalidated on login, refresh, and every protected request |
| ONYX access/refresh JWT validity | Issued by the Worker with `ONYX_JWT_SECRET` only after D1 checks; expiry and `token_revocations` rechecked on use |
| `role`, `is_admin`, `organization_id`, `class`, `parent_user_id` in any request body, query, or client store | **Never trusted as authority** — client-supplied values are ignored for authorization decisions |

Public authentication failures return stable error codes only (`INVALID_CREDENTIALS`,
`ALLFATHER_REQUIRED`, `ALLFATHER_NOT_PROVISIONED`, `INVALID_SUPABASE_TOKEN`,
`INVALID_REFRESH_TOKEN`, `UNAUTHORIZED`). The verifier's detailed reason is logged server-side as
a structured event (`allfather_subject_rejected`, `allfather_token_rejected`) that never contains
tokens, passwords, OAuth codes, or secrets.

## All-Father password provisioning (owner-controlled)

GitHub Actions secrets are inputs to the smoke workflow; they do not provision D1. The canonical principal is seeded with a non-usable placeholder hash until an owner-controlled password provisioning operation replaces it.

Run `npm run provision:allfather-password -- --environment test` only against the local D1 database after applying migrations. The command validates the unique canonical principal, explicit role/admin flags, active account and active system organization, writes a salted Argon2id hash and a non-sensitive audit event, and verifies the stored hash.

Production provisioning is never run by CI or deployment. It requires explicit authorization and both `--approve-production __allfather__` and `--expect-database onyx-free-db`; it changes only the existing canonical account's password hash and does not create or reactivate accounts. Do not run production provisioning without owner approval.

The historical `users.supabase_user_id` column and migrations remain for schema-history compatibility only. They are not read or written by the current authentication flow.

## Current state

Phase 1 has converted the Worker from an origin proxy into an origin-free application boundary:

- `/health` is implemented locally in the Worker.
- `/ready` checks the D1 binding when it is configured.
- There is no `ONYX_ORIGIN_URL` and no upstream `fetch()` to Render.
- D1 migrations create the Worker’s authoritative users, organization, event, and audit tables; Supabase Auth stores credentials and authenticates identities.
- Unported ONYX API routes return `501 API_MIGRATION_IN_PROGRESS` rather than silently routing traffic to the unreachable legacy backend.

This is intentional. The Worker is not claimed to be a complete replacement until each API contract has been ported and verified.

## Deployment

The GitHub Actions workflow `.github/workflows/deploy-cloudflare-edge.yml` creates or reuses the free D1 database, applies checked-in migrations, configures Worker secrets, deploys the Worker, and checks `/health`, `/ready`, and anonymous access to a protected route. It requires Cloudflare and Supabase credentials configured as GitHub Actions secrets. The production CORS allowlist is supplied through the required `ONYX_CORS_ORIGINS` GitHub Actions secret. The deployment workflow fails closed when the secret is missing or contains a wildcard; keep the allowlist limited to the exact trusted origins. The workflow is manually dispatchable; deployment credentials are never committed to the repository.

### Local development

Local development uses the committed `wrangler.dev.toml`. It carries the D1
binding (`DB`, `migrations_dir = "migrations"`) and the non-secret public `[vars]`
the Worker reads, so a fresh checkout can boot the Worker and apply migrations
with no extra setup. Its `database_id` is a fixed **local-only placeholder** that
is deliberately not the production id, so no command run against this config can
target the remote database.

```bash
npm install
npm run d1:local   # create + apply all migrations to the local D1, including 0005_admin_settings.sql
npm run dev        # serve the Worker with the local D1 binding
```

`GET /ready` should then return `{"status":"ok","database":"d1","schema":"ok"}`;
`/health` and the anonymous `401` on a protected route are the same checks the
deploy workflow runs. `npm run check` (used by CI) validates the bundle built from
this config.

Do not commit secrets. `ONYX_JWT_SECRET`, `HF_S3_ACCESS_KEY_ID`, and `HF_S3_SECRET_ACCESS_KEY`
belong in an untracked `.dev.vars` (see `.dev.vars.example`); production sets them
with `wrangler secret put`. Remote commands (`npm run d1:remote`, `npm run deploy`)
use the CI-materialized `wrangler.deploy.toml`, which is not committed.

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
