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
        +---- Supabase Auth (Google for All-Father; email/password for Admin/Staff)
        |
        +---- Hugging Face object storage for existing blob data
```

Cloudflare Workers Free is the runtime boundary. No custom domain is required: the `workers.dev` hostname is sufficient. The migration deliberately does not use Cloudflare Containers, Workers Paid, Render, or another paid/container-only service.

## Locked authentication and authorization model

This model is binding for the migration:

- **All-Father only:** Google sign-in through Supabase Auth. The Worker verifies the signed Supabase JWT, resolves the bearer token through Supabase Auth, requires the returned user ID to match the JWT subject, requires a confirmed email, and matches the configured All-Father email. Supabase identity is not used to authenticate Admin or Staff accounts.
- **Organization Admin:** ONYX username or email plus password. Credentials are verified against the Argon2id password hash stored in D1.
- **Staff:** ONYX username or email plus password. Accounts are created, deactivated, and managed by an authorized organization Admin; staff cannot self-register or elevate roles.
- **D1 is authoritative** for ONYX users, password hashes, account status, roles, organization membership, and application authorization.
- **All-Father provisions organizations and Admins. Admins manage Staff only within their own organization.** The Worker must enforce these rules server-side.

Authentication endpoints:

- `POST /api/auth/supabase`: exchange a valid Supabase All-Father bearer token for an ONYX session.
- `POST /api/auth/login`: verify an ONYX username/email and password against D1, then issue an ONYX session.
- `POST /api/auth/refresh`: rotate a valid ONYX refresh token.
- `POST /api/auth/logout`: revoke supplied ONYX tokens.

Browser sign-in uses Supabase's **Authorization Code Flow with PKCE** (`crates/bins/admin-shell/ui/src/auth/allfatherOAuth.ts`): the Admin shell creates the verifier, its S256 challenge and a CSRF nonce, exchanges the code with the publishable key only, and scrubs the code from the URL. The Worker exchange endpoint is unchanged — it still accepts a bearer token and never learns any OAuth secret.

The D1 migrations own this model: `0001_foundation.sql` creates the base tables, `0002_auth_hierarchy.sql` adds `email`/`role` and seeds the single inert `__allfather__` principal, `0003_supabase_identity.sql` owns `users.supabase_user_id`, and `0004_auth_schema_repair.sql` idempotently reconciles the canonical indexes and principal (see `reports/auth-repair/03-d1-migrations.md`). Admin and Staff passwords are Argon2id hashes in D1; their accounts are not provisioned in Supabase.

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

## All-Father subject provisioning (owner-only, out-of-band)

`users.supabase_user_id` is the trust binding between a verified Supabase subject and the single
ONYX All-Father principal. The migrations create it `NULL`, and the only supported writer is the
administrative command `scripts/provision-allfather.mjs` (alias `npm run provision:allfather`).
Login never writes it, no migration writes it, and no client can supply it.

The command is fail-closed. It refuses to run when:

- the canonical principal `__allfather__` is missing, duplicated, inactive, not `is_admin`, or its
  `role` is not the explicit `ALL_FATHER` value (a username/email/Google identity alone never grants it);
- the supplied subject is not a Supabase Auth user UUID (an email address is rejected before any SQL);
- the subject is already mapped to a *different* ONYX principal (no double-binding);
- the principal already has a different subject and `--rebind` is not passed;
- `--environment production` is used without `--approve-production <subject>` (or `__allfather__` for a revoke);
- `--expect-database` does not match `--database`.

Every accepted change writes an `audit_log` row (`allfather.provision` / `allfather.revoke`) and is
idempotent: re-running with the same subject is a no-op that reports `already-provisioned`.
Credentials, tokens, and `password_hash` values are never read, written, or logged.

The npm alias runs the same script from this directory (`deploy/cloudflare/edge-gateway`), so it
inherits the same working-directory assumption. Pass the script flags after `--`:

```bash
npm run provision:allfather -- --environment production --config wrangler.deploy.toml \
  --database onyx-free-db --expect-database onyx-free-db \
  --supabase-user-id <SUPABASE_UUID> --dry-run --json
```

The `node scripts/provision-allfather.mjs ...` examples below are equivalent (and are the form to use
when this directory is not the current working directory).

### Runbook — first-time production provisioning

1. **Read-only preflight** (never prints hashes/tokens). Confirm exactly one privileged row and no
   existing mapping:

   ```bash
   npx wrangler d1 execute onyx-free-db --remote --config wrangler.deploy.toml --json \
     --command "SELECT id, username, role, is_active, is_admin, supabase_user_id FROM users"
   ```

   If there is more than one `ALL_FATHER` row or the row is not `__allfather__`, **stop** and resolve
   it manually; the command will refuse (`PRINCIPAL_AMBIGUOUS`) rather than merge or delete rows.

2. **Obtain the Supabase user UUID** from the owner's Supabase project (Authentication → Users) for
   the Google identity. Do not derive it from the email address.

3. **Dry run** (preflight + plan, no writes):

   ```bash
   node scripts/provision-allfather.mjs --environment production --config wrangler.deploy.toml \
     --database onyx-free-db --expect-database onyx-free-db \
     --supabase-user-id <SUPABASE_UUID> --dry-run --json
   ```

4. **Apply with explicit owner approval** (required only after the D1 migrations and the Worker are
   deployed, since login stays denied until this mapping exists):

   ```bash
   node scripts/provision-allfather.mjs --environment production --config wrangler.deploy.toml \
     --database onyx-free-db --expect-database onyx-free-db \
     --supabase-user-id <SUPABASE_UUID> --approve-production <SUPABASE_UUID> \
     --operator "<owner>"
   ```

5. **Expected success indicators:** `status: provisioned`; `SELECT supabase_user_id FROM users WHERE
   id='__allfather__'` returns the UUID; `audit_log` contains one `allfather.provision` row; the
   Google sign-in then returns an ONYX session instead of `403 ALLFATHER_NOT_PROVISIONED`.

### Runbook — identity replacement (break-glass)

For a lost or replaced Google identity, revoke first, then provision the new subject. Both steps are
audited and require approval:

```bash
node scripts/provision-allfather.mjs --environment production --action revoke \
  --approve-production __allfather__ --operator "<owner>"
node scripts/provision-allfather.mjs --environment production --config wrangler.deploy.toml \
  --database onyx-free-db --supabase-user-id <NEW_UUID> --rebind \
  --approve-production <NEW_UUID> --operator "<owner>"
```

Note: local Wrangler D1 state is keyed by the `database_id` in the config, not the database name —
use a distinct `database_id` for each disposable environment, and never point `--remote` at a
database you have not identified with `--expect-database`.

## Current state

Phase 1 has converted the Worker from an origin proxy into an origin-free application boundary:

- `/health` is implemented locally in the Worker.
- `/ready` checks the D1 binding when it is configured.
- There is no `ONYX_ORIGIN_URL` and no upstream `fetch()` to Render.
- D1 migrations create the Worker’s authoritative users, organization, event, and audit tables; Supabase Auth stores credentials and authenticates identities.
- Unported ONYX API routes return `501 API_MIGRATION_IN_PROGRESS` rather than silently routing traffic to the unreachable legacy backend.

This is intentional. The Worker is not claimed to be a complete replacement until each API contract has been ported and verified.

## Deployment

The GitHub Actions workflow `.github/workflows/deploy-cloudflare-edge.yml` creates or reuses the free D1 database, applies checked-in migrations, configures Worker secrets, deploys the Worker, and checks `/health`, `/ready`, and anonymous access to a protected route. It requires Cloudflare and Supabase credentials configured as GitHub Actions secrets. The workflow is manually dispatchable; deployment credentials are never committed to the repository.

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

Do not commit secrets. `ONYX_JWT_SECRET`, `SUPABASE_SECRET_KEY`,
`SUPABASE_PUBLISHABLE_KEY`, `HF_S3_ACCESS_KEY_ID`, and `HF_S3_SECRET_ACCESS_KEY`
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
