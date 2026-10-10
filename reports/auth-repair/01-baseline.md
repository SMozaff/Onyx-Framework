# ONYX Authentication Repair — Phase 1 Baseline

**Assessment date:** 2026-10-10 (re-verification revision)
**Repository:** https://github.com/SMozaff/Onyx-Framework
**Branch examined:** `main` (local worktree: `freebuff/2aea8dd80399fb79944e189c`, clean, identical SHA)
**Baseline commit:** `2066216ee08a7222df6bc64f9088a87b3c4791ce`
**Commit URL:** https://github.com/SMozaff/Onyx-Framework/commit/2066216ee08a7222df6bc64f9088a87b3c4791ce
**Scope:** Read-only inspection plus this report file only. No production systems, migrations, secrets, Worker settings, or D1 data were changed.

> **Revision note.** The original Phase 1 baseline (commit `12b7568`) was recorded against
> `c6dcbca9771206a70429a48ecaee5ebc734dce65`. `main` has since advanced: PR #172 merged the
> Phase 2 auth-contract reconciliation, and draft PR #173 (Phase 3) is open. This revision
> re-verifies every prior observation against the current tree and adds live read-only
> evidence from the deployed Worker. The prior report remains in Git history at `12b7568`.

## 1. Repository state, CI, and pull requests

- `main` tip and the examined worktree are both `2066216ee08a7222df6bc64f9088a87b3c4791ce`
  (merge of PR #172, "docs(auth): reconcile ONYX authentication contract", 2026-10-10 16:46 UTC).
  Local `git status --porcelain` is empty (clean tree, no stashes).
- **Open PR:** [#173 (draft) — "docs(auth): record D1 migration repair blocker"](https://github.com/SMozaff/Onyx-Framework/pull/173),
  branch `auth-repair/03-d1-migrations` at `034b429b894e805b29623d8005dc8dee160c5431`.
  It adds `reports/auth-repair/03-d1-migrations.md` and is blocked pending owner authorization
  for read-only D1 ledger/schema inspection. No migration SQL was changed in that PR.
- CI: [Security run 38069757800](https://github.com/SMozaff/Onyx-Framework/actions/runs/38069757800)
  for PR #173 was `in_progress` at inspection. Repo history shows 1851 workflow runs total.
  Security run [38062963607](https://github.com/SMozaff/Onyx-Framework/actions/runs/38062963607)
  for the earlier baseline commit completed successfully; it is automated validation only and
  proves nothing about deployed authentication.

## 2. Repository instructions and constraints

- No `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `SECURITY.md`, or `CODEOWNERS` exists anywhere
  in the inspected tree. No local-execution prohibition was found; verification is limited by
  missing dependencies/credentials, not by instruction.
- Binding documents:
  - `docs/SECURITY_AUTHENTICATION_MODEL.md` — "authoritative security requirement"; now includes a
    2026-10-10 "Implementation reconciliation" addendum recording known implementation gaps.
  - `docs/Onyx Auth system.md` — three-tier identity model (All-Father / Org Admin / Staff).
  - `deploy/cloudflare/edge-gateway/README.md` — locked model, endpoint contract, and the
    migration rule that routes may only go live after automated tests exist.

## 3. Locked intended model (unchanged)

- **All-Father:** Google identity `so.muzaff@gmail.com` via Supabase Auth only. Worker must verify
  JWT signature/issuer/expiry/subject, resolve the token through Supabase Auth, require a confirmed
  email matching the designated identity, map to a **provisioned** ONYX principal, and authorize
  server-side. Google authentication alone must never grant All-Father privileges.
- **Organization Admin / Staff:** ONYX-managed username-or-email + password; Argon2id hashes and
  account state in D1; no third-party IdP.
- **D1 is authoritative** for users, roles, active state, organization membership, and all
  authorization decisions. UI gates are not security boundaries.
- All-Father provisions organizations and Admins; Admins manage Staff only within their own org.

## 4. Confirmed findings (verified at `2066216`)

### 4.1 Duplicate/conflicting `0002` migrations — confirmed

`deploy/cloudflare/edge-gateway/migrations/` contains four files with three distinct prefixes:

- `0001_foundation.sql`
- `0002_auth_hierarchy.sql` — creates `organizations`, adds `email` + `role`, seeds org
  `__allfather__` and user `__allfather__`/'allfather'.
- `0002_onyx_identity_model.sql` — adds `email` + `role` + `supabase_user_id`, creates
  `organizations`, seeds a **different** org `__onyx_root__` and user `__onyx_allfather__`/'allfather'.
- `0003_supabase_identity.sql` — adds `supabase_user_id` **again** and creates a differently named
  unique index (`idx_users_supabase_user_id` vs `idx_users_supabase_user`).

Repeated `ALTER TABLE users ADD COLUMN email/role/supabase_user_id` cannot all succeed on one
database; whichever files the D1 migration ledger does not yet record will fail when applied.
Which files are recorded in production D1 is **unknown** (blocked on owner-authorized read-only
ledger access — see draft PR #173).

Additionally, `deploy/cloudflare/edge-gateway/d1/` contains a second, partially duplicated copy of
the migration set (`0001_foundation.sql`, `0002_auth_hierarchy.sql`). Only `migrations/` is
referenced by `wrangler.deploy.toml` generation in `.github/workflows/deploy-cloudflare-edge.yml`
(`migrations_dir = "migrations"`); the `d1/` copies are unreferenced duplicate state that can drift.

Two seed identities compete for the same lowercase username `allfather` (unique index
`idx_users_username_lower`): `__allfather__` (org `__allfather__`) and `__onyx_allfather__`
(org `__onyx_root__`). `INSERT OR IGNORE` does not protect against this: the unique username index
causes the second insert to be silently ignored, so the surviving principal depends on file order.

### 4.2 Worker auth routes — source-confirmed at current SHA

`deploy/cloudflare/edge-gateway/src/index.ts`:

- Route dispatch (lines 245–248): `POST /api/auth/login`, `POST /api/auth/supabase` (legacy alias
  `/api/auth/clerk`), `POST /api/auth/refresh`, `POST /api/auth/logout`.
- `/api/auth/supabase` (lines 135–157) now verifies the Supabase JWT, resolves the bearer token
  through Supabase Auth (`src/supabase.ts:37–101`: signature via JWKS, algorithm allowlist
  RS256/ES256, issuer `${SUPABASE_URL}/auth/v1`, expiry, `identity.id === claims.sub`,
  `email_confirmed_at`, email match against `env.ONYX_ALLFATHER_EMAIL || so.muzaff@gmail.com`
  at line 35). This is stronger than the Phase 1 snapshot.
- **Contract gap (confirmed):** lines 151–154 still `UPDATE users SET supabase_user_id=?` when the
  stored mapping differs from the presented subject. The locked contract addendum in
  `docs/SECURITY_AUTHENTICATION_MODEL.md` explicitly forbids this ("Login must not create, update,
  or replace users.supabase_user_id; a subject mismatch must be denied without a database write").
- **Principal lookup (confirmed):** line 149 selects the principal with
  `WHERE LOWER(username)=LOWER('allfather')`, i.e. by mutable username rather than a stable
  provisioned ID; `roleForUser()` (lines 108–113) also infers `ALL_FATHER` from the literal
  username `allfather`. The two competing seed rows make this lookup order/ledger-dependent.
- **Raw provider/verifier errors leak to clients:** line 155 returns the caught exception message
  verbatim in the public JSON error body, contrary to the contract's "stable, generic error codes" rule.
- `/ready` (line 243) performs only `SELECT 1` and reports `database:"d1"`; it does **not** validate
  the required auth schema (`organizations`, `users.email`, `users.role`, `users.supabase_user_id`,
  seeded All-Father principal). Confirmed trivial.
- ONYX login (lines 120–128) looks up `users` by case-insensitive username or email, verifies
  Argon2id, rejects inactive accounts and rejects password login for `ALL_FATHER`.
- Session revalidation (lines 88–96) re-reads `is_active`, `organization_id`, and role from D1 on
  every protected request; refresh rotates via `token_revocations`. Access token 1 h, refresh 7 d.
- Unported routes return `501 ROUTE_NOT_MIGRATED` **including** a `message` field (line 263).

### 4.3 Admin Shell login — source-confirmed

`crates/bins/admin-shell/ui/src/pages/Login.tsx`:

- Line 31: extracts `access_token` from the **URL fragment**; lines 40–42 post it to
  `/api/auth/supabase` as a bearer token.
- Line 68: `target.searchParams.set("flow_type", "implicit")` — **implicit flow is explicitly
  requested in source.** Provider-side configuration is not verifiable from the repository.
- Lines 58–59: Supabase URL/publishable key come from `VITE_SUPABASE_URL` /
  `VITE_SUPABASE_PUBLISHABLE_KEY`; if unset the UI shows a not-configured message.
- Legacy Clerk code remains: `crates/bins/admin-shell/ui/src/auth/clerk.ts` contains a hard-coded
  fallback Clerk publishable key (public-by-design value) and is dead weight against the locked model.
- Session tokens are stored client-side via `src/utils/auth.ts` / `src/stores/authStore.ts` (localStorage-backed).

### 4.4 Deployment and CI wiring — source-confirmed

- `.github/workflows/deploy-cloudflare-edge.yml`:
  - Triggers: `workflow_dispatch` and pushes to `migration/cloudflare-free-worker` (not `main`).
  - Creates/reuses D1 DB named `onyx-free-db`, materializes `wrangler.deploy.toml` with binding `DB`
    and `migrations_dir = "migrations"`, applies migrations `--remote`, sets secrets, deploys, then
    verifies `/health`, `/ready` (expects 200), and expects **401** from
    `GET /api/users/hierarchy` anonymously.
  - `ONYX_ALLFATHER_EMAIL` and the Supabase project URL are hard-coded in the workflow file.
- `.github/workflows/cloudflare-worker-check.yml`: `npm install` + `npm run check`
  (`wrangler deploy --dry-run`) only — a bundle validation, **not** a test run.
- `.github/workflows/live-deployment-smoke.yml`: live `/health` + `/ready` probe on pushes to
  `migration/cloudflare-free-worker`; also expects 401 from `/api/users`.
- `.github/workflows/migrate-worker-to-supabase.yml`: deterministic Python source rewrite that
  commits and pushes to `migration/cloudflare-free-worker` (write-enabled workflow).
- `deploy/cloudflare/edge-gateway/package.json` defines `d1:local` / `d1:remote` against `migrations/`.

### 4.5 NEW: live deployed Worker does not match current source — confirmed (read-only probes)

Unauthenticated GET requests (no credentials, no state change) against the production URL recorded
in the workflows, `https://onyx-framework.soheil-mozaffari.workers.dev`:

| Probe | Result |
|---|---|
| `GET /health` | `200 {"status":"ok","service":"onyx-cloudflare-worker","runtime":"cloudflare-workers-free"}` |
| `GET /ready` | `200 {"status":"ok","service":"onyx-cloudflare-worker","database":"d1"}` |
| `GET /api/users/hierarchy` (anonymous) | **`501 {"error":"ROUTE_NOT_MIGRATED","route":"/api/users/hierarchy"}`** (no `message` field) |
| `POST /api/auth/supabase` (no token) | **`501 {"error":"ROUTE_NOT_MIGRATED","route":"/api/auth/supabase"}`** |

Interpretation:

- The **deployed Worker is an older build than `2066216`.** Current source implements both routes;
  an anonymous `GET /api/users/hierarchy` would return `401 UNAUTHORIZED`, and the 501 body in
  current source always includes a `message` field (index.ts:263). The deployed body has none.
- The deployed build therefore predates the auth-route implementation and the
  "Verify protected API rejects anonymous access" gate in the deploy workflow (which would have
  failed on a 501 where it expects 401) — meaning **the currently deployed Worker was not produced
  by a successful run of the current deploy workflow**, or was deployed before that gate existed.
  The exact deployed commit is **unknown** (no Workers version metadata access).
- `/ready` is live and reports `database:"d1"`, so a D1 binding exists in production, but readiness
  still validates nothing about the auth schema.
- `/api/auth/login`, `/api/auth/refresh`, `/api/auth/logout` behavior on the deployed build is
  **unknown** (not probed: probing them would require credential-bearing requests).
- The Supabase → Worker → D1 → ONYX session exchange has **not** been demonstrated end to end.

## 5. Unknowns / not verifiable with current access

- Deployed Worker version/commit, environment variables, and secret presence (no Cloudflare API
  credentials are available in this environment; `wrangler` is unauthenticated).
- Production D1 database ID, migration ledger (`d1_migrations`), and actual auth schema/columns.
  Whether `organizations`, `users.email`, `users.role`, `users.supabase_user_id` exist in production.
- Which All-Father seed principal (`__allfather__` vs `__onyx_allfather__`) exists in production, if any.
- Supabase project OAuth settings (redirect URLs, flow type) — provider console not accessible.
- Whether any successful production deployment of the current Worker source ever occurred.
- Whether the last successful deploy workflow run applied all four migrations cleanly.
- Real end-to-end authentication outcomes for any role. **No login flow is claimed to work.**

## 6. Tests and CI gaps

- Rust API reference: `crates/bins/api-server/tests/` (`auth_refresh.rs`, `mobile_access_gate.rs`,
  `mobile_observer_capability.rs`, `observer_read_routes.rs`, `staff_loan_authorization.rs`,
  `team_leader_precheck_authorization.rs`, `user_hierarchy_admin_routes.rs`) — these test the Axum
  service, not the Cloudflare Worker.
- Worker: **no test files exist**; `npm run check` only dry-run-bundles. There are no automated
  tests for `/api/auth/supabase`, `/api/auth/login`, refresh rotation, revocation, tenant isolation,
  migration application, or readiness schema validation.
- Admin Shell UI: no automated login test; Playwright specs exist under `web-ui/tests` and
  `mobile-pwa/tests` but do not cover the Supabase→Worker exchange.
- CI verifies only that the bundle compiles and (for live smoke) that the worker answers `/health`.

## 7. Files likely to change in later phases

1. `deploy/cloudflare/edge-gateway/migrations/0002_auth_hierarchy.sql`
2. `deploy/cloudflare/edge-gateway/migrations/0002_onyx_identity_model.sql`
3. `deploy/cloudflare/edge-gateway/migrations/0003_supabase_identity.sql`
4. `deploy/cloudflare/edge-gateway/d1/*` (duplicate copies — consolidate or delete)
5. `deploy/cloudflare/edge-gateway/src/index.ts` (remove `supabase_user_id` auto-bind write;
   stable principal lookup; generic public errors)
6. `deploy/cloudflare/edge-gateway/src/supabase.ts`
7. `crates/bins/admin-shell/ui/src/pages/Login.tsx` (flow type/callback handling)
8. `crates/bins/admin-shell/ui/src/auth/clerk.ts` (legacy removal)
9. `.github/workflows/deploy-cloudflare-edge.yml`
10. `.github/workflows/cloudflare-worker-check.yml` (real Worker auth/migration tests)
11. `deploy/cloudflare/edge-gateway/package.json` + new Worker test files
12. `reports/auth-repair/03-d1-migrations.md` (in flight via draft PR #173)

## 8. Proposed sequence for phases 2–8 (revised against current state)

2. ✅ **Done** (PR #172): contract and route matrix reconciled; gaps documented.
3. **Migration/schema reconciliation (in flight, blocked):** obtain owner-authorized **read-only**
   D1 ledger + schema evidence, then choose one of the two history-dependent repair paths documented
   in draft PR #173. Forward-only, additive repair preferred; do not rewrite applied history.
4. **Supabase identity binding:** remove the `supabase_user_id` write from login (index.ts:151–154),
   key the principal on a stable ID, resolve the implicit-flow callback decision, delete legacy Clerk code.
5. **ONYX credential/session path:** refresh-rotation and revocation tests, inactive-account
   rejection, generic error responses.
6. **Readiness and observability:** extend `/ready` to verify required auth tables/columns and the
   provisioned All-Father principal without exposing configuration.
7. **Automated verification and deployment guardrails:** add Worker-level auth/migration tests;
   make deploy depend on them; confirm the anonymous-401 gate matches the deployed route set.
8. **Staged production verification with owner approval:** record pre-state, apply approved
   forward migrations, redeploy, then verify `/health`, `/ready`, anonymous-401, and a real,
   authorized Supabase→Worker→D1 session round trip plus negative authorization cases.

Each phase must stop if its acceptance criteria fail.

## 9. Safety and owner approvals required

- **No production mutation occurred in this phase.** Only unauthenticated GET requests and one
  token-less POST (which returned `501 ROUTE_NOT_MIGRATED` without touching the database) were sent
  to the public Worker URL. No Cloudflare or Supabase credentials are present in this environment.
- Explicit production-owner approval is required before: applying D1 migrations; changing Worker
  bindings, config, or secrets; changing Supabase OAuth settings/redirect URLs; deploying a Worker
  version; creating/modifying/deleting production identities; changing production traffic.
- Read-only D1 ledger/schema inspection (already requested in draft PR #173) also requires explicit
  owner authorization and must never select hash, token, or secret values.
- Never print or include secret values in logs or reports.

## 10. Conclusion

Baseline re-verified at `2066216`. The two most consequential confirmed facts are:
(1) the migration set still contains conflicting duplicate `0002` files that seed **two different**
All-Father principals and double-add identity columns, and
(2) the **deployed production Worker is an older build that does not implement the auth routes at
all** — `/api/auth/supabase` currently answers `501 ROUTE_NOT_MIGRATED` in production, so no
Supabase sign-in can work today regardless of the source-level improvements merged in PR #172.
Implementation is intentionally not started by this phase; the next actionable step is the
owner-authorized read-only D1 inspection already staged in draft PR #173, followed by a migration
repair, then a real deployment of the current source with verification gates.

*This report is uncommitted in the local worktree; commit it as `docs(auth)` if it should be
preserved on a branch.*
