# ONYX Authentication Repair — Phase 1 Baseline

**Assessment date:** 2026-10-10  
**Repository:** [SMozaff/Onyx-Framework](https://github.com/SMozaff/Onyx-Framework)  
**Branch examined:** `main`  
**Baseline commit:** `c6dcbca9771206a70429a48ecaee5ebc734dce65`  
**Commit URL:** https://github.com/SMozaff/Onyx-Framework/commit/c6dcbca9771206a70429a48ecaee5ebc734dce65  
**Scope:** Read-only baseline investigation, plus this report file only. No production systems were changed.

## 1. Repository state and instructions

- GitHub repository metadata identifies `main` as the default branch; latest commit at inspection was `c6dcbca9771206a70429a48ecaee5ebc734dce65` (2026-10-10 15:16:49 UTC), message `fix(i18n): regenerate dictionaries from canonical catalogs`.
- No open pull requests were returned by the GitHub API at inspection.
- The local working-tree state cannot be determined through the repository API; it is **unknown**, not assumed clean.
- Root `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, and `SECURITY.md` were not found at the examined branch paths. The repository does contain `docs/SECURITY_AUTHENTICATION_MODEL.md`, `docs/Onyx Auth system.md`, `DECISIONS.md`, and the Cloudflare deployment README. No instruction files were confirmed in the inspected tree; nested instruction-file absence was not exhaustively proven.
- Authoritative model: `docs/SECURITY_AUTHENTICATION_MODEL.md` explicitly says it governs the migration. `docs/Onyx Auth system.md` agrees on the intended three-level model.

## 2. Locked intended model

- **All-Father:** Google via Supabase Auth only; Worker must verify JWT signature, issuer, expiry and subject, verify the identity through Supabase Auth, require the designated confirmed email, map to a provisioned ONYX principal, and authorize server-side.
- **Organization Admin and Staff:** ONYX-managed username/email + password; Argon2id hashes and account state in D1.
- **Provisioning:** All-Father provisions organizations and Admins; Admins manage Staff only within their organization.
- **Authority source:** D1 is authoritative for ONYX users, roles, active state, organization membership and authorization. UI gates are not security boundaries.

Evidence: `docs/SECURITY_AUTHENTICATION_MODEL.md`; `docs/Onyx Auth system.md`; `deploy/cloudflare/edge-gateway/README.md`.

## 3. Confirmed findings

### 3.1 Migration numbering and schema conflicts — confirmed

The current tree contains:
- `deploy/cloudflare/edge-gateway/migrations/0001_foundation.sql`
- `deploy/cloudflare/edge-gateway/migrations/0002_auth_hierarchy.sql`
- `deploy/cloudflare/edge-gateway/migrations/0002_onyx_identity_model.sql`
- `deploy/cloudflare/edge-gateway/migrations/0003_supabase_identity.sql`

Two files share migration prefix `0002`. This is a confirmed naming/ordering conflict whose actual effect depends on the migration runner's discovery and ordering logic; do not assume both apply successfully.

Schema overlap is confirmed by file contents:
- `0002_auth_hierarchy.sql` adds `email` and `role`, creates `organizations`, and seeds `__allfather__` / `__allfather__` organization.
- `0002_onyx_identity_model.sql` also adds `email` and `role`, adds `supabase_user_id`, creates `organizations`, and seeds a different `__onyx_allfather__` / `__onyx_root__` principal.
- `0003_supabase_identity.sql` adds `supabase_user_id` again and creates a differently named unique index.

If both overlapping migrations run on the same database, repeated `ALTER TABLE ADD COLUMN` operations are likely to fail. Exact deployed schema and migration history are **not verified** in this phase.

### 3.2 Worker auth routes and readiness — source-confirmed

In `deploy/cloudflare/edge-gateway/src/index.ts`:
- Routes exist for `POST /api/auth/login`, `POST /api/auth/supabase` (and legacy `/api/auth/clerk` alias), `POST /api/auth/refresh`, and `POST /api/auth/logout` (route dispatch around lines 245–248).
- `/ready` performs only `SELECT 1` against the D1 binding and reports database availability (around line 243). It does **not** validate required authentication tables or columns.
- Login queries `users` fields including `email`, `role`, and account status (around line 125); refresh also queries current account state (around line 142 in the current file, verify exact line positions against the baseline commit when modifying).
- Supabase exchange looks up an All-Father username and references `supabase_user_id`; current source can update that mapping when the presented identity differs (around lines 149–153). The intended model requires explicit provisioned-principal mapping and server-side authorization; the safety and binding behavior needs dedicated review before deployment.
- `src/supabase.ts` checks token structure, accepted signing algorithm, issuer and expiry, fetches JWKS, and includes a Supabase Auth user lookup path. This source inspection does not establish an end-to-end successful login.

### 3.3 Admin Shell login — source-confirmed

`crates/bins/admin-shell/ui/src/pages/Login.tsx` contains an All-Father mode that extracts `access_token` from the URL hash and posts it as a bearer token to `/api/auth/supabase`. The presence of this UI path does not prove OAuth flow correctness or that the deployed Worker accepts it. The code should be reviewed for implicit-flow use; the hash-token callback is consistent with an implicit-style token return, but the complete provider configuration and live redirect behavior are **not verified**.

### 3.4 Deployment and CI wiring — source-confirmed

- `.github/workflows/deploy-cloudflare-edge.yml` deploys the Worker and has production-affecting behavior. It triggers manually and on pushes to `migration/cloudflare-free-worker` for matching paths, not ordinary `main` pushes.
- `.github/workflows/cloudflare-worker-check.yml` runs `npm install` and `npm run check` for Worker changes and relevant PRs.
- `.github/workflows/migrate-worker-to-supabase.yml` runs a deterministic migration script and can commit/push to `migration/cloudflare-free-worker`.
- Latest visible CI run for the baseline commit: [Security workflow run 38062963607](https://github.com/SMozaff/Onyx-Framework/actions/runs/38062963607), completed successfully. This is not proof that authentication integration or deployment succeeded.
- A workflow dispatch for [ONYX Signed Release run 38064005600](https://github.com/SMozaff/Onyx-Framework/actions/runs/38064005600) was cancelled. It is not an authentication test result.
- No open PRs were returned at inspection.

## 4. Unknowns and access limitations

The following were not verified and must remain unknown:
- Deployed Worker metadata/version and its actual environment/bindings.
- Whether the deployed Worker is bound to the expected D1 database by database ID/name.
- Deployed D1 schema, migration history, presence of `organizations`, `users.email`, `users.role`, and `users.supabase_user_id`.
- Whether the intended All-Father principal exists in deployed D1, and which of the competing seed identities is present.
- Production authentication outcomes and real authorized end-to-end login.
- Production OAuth provider settings and whether implicit flow is currently configured.
- Whether the deploy workflow's most recent production execution applied all migrations successfully.
- Local working-tree state and uncommitted changes.

No authorized read-only Cloudflare metadata or SQL access was exercised during this baseline. No production data or secrets were queried.

## 5. Relevant tests and gaps

Repository test inventory includes:
- API server: `crates/bins/api-server/tests/auth_refresh.rs`, `mobile_access_gate.rs`, `mobile_observer_capability.rs`, `observer_read_routes.rs`, plus hierarchy and role-specific authorization tests.
- Admin Shell UI: `crates/bins/admin-shell/ui/src/pages/Login.tsx`; no directly named Admin Shell login test was identified in the inspected path inventory.
- Worker: `.github/workflows/cloudflare-worker-check.yml` validates the bundle with `npm run check`; no dedicated Worker auth test file was identified in the inspected Worker tree.
- PWA/browser auth tests exist under `mobile-pwa/tests` and `web-ui/tests`, but they do not establish the Cloudflare Worker/D1 auth exchange end to end.
- CI evidence is automated validation only. A real deployed Supabase → Worker → D1 → ONYX session round trip, negative authorization cases, and deployed-schema readiness check remain unverified.

## 6. Files likely to change in later phases

Primary scope:
1. `deploy/cloudflare/edge-gateway/migrations/0002_auth_hierarchy.sql`
2. `deploy/cloudflare/edge-gateway/migrations/0002_onyx_identity_model.sql`
3. `deploy/cloudflare/edge-gateway/migrations/0003_supabase_identity.sql`
4. `deploy/cloudflare/edge-gateway/src/index.ts`
5. `deploy/cloudflare/edge-gateway/src/supabase.ts`
6. `deploy/cloudflare/edge-gateway/README.md`
7. `deploy/cloudflare/edge-gateway/wrangler.toml` (only if binding/config declaration changes are needed)
8. `crates/bins/admin-shell/ui/src/pages/Login.tsx`
9. `crates/bins/admin-shell/ui/src/auth/clerk.ts` and `crates/bins/admin-shell/ui/src/stores/authStore.ts` if shared auth/session behavior needs correction
10. `.github/workflows/deploy-cloudflare-edge.yml`
11. `.github/workflows/cloudflare-worker-check.yml`
12. `deploy/cloudflare/edge-gateway/package.json` and new focused Worker auth/migration tests if required.

Do not widen this list to unrelated application features or localization work without new evidence.

## 7. Proposed phases 2–8

2. **Migration/schema reconciliation:** establish a single monotonic migration sequence, determine the deployed schema/history through authorized read-only access, design a forward-only repair and rollback plan. Do not rewrite already-applied migration files without migration-history evidence.
3. **Supabase identity verification:** verify JWT signature/issuer/expiry/subject and verified user details; remove identity auto-binding or fallback behavior unless explicitly justified by a safe provisioning contract.
4. **ONYX credential/session path:** audit username/email lookup, Argon2id verification, inactive-account rejection, refresh rotation/revocation, logout and session role revalidation.
5. **Authorization and tenant boundaries:** test All-Father-only operations, Admin-only provisioning, organization isolation, and denial of privilege escalation.
6. **Readiness and observability:** make readiness verify required auth schema/columns and safe D1 queries; do not expose sensitive configuration.
7. **Automated verification and deployment guardrails:** add Worker-level migration/auth tests and negative cases; make deployment dependent on checks and migration validation.
8. **Staged production verification:** obtain explicit owner approval, take backups/record migration state, apply only approved forward migrations, verify endpoints and authorized end-to-end flows, document rollback triggers.

Each phase should produce evidence and stop before the next phase if its acceptance criteria fail.

## 8. Safety and owner approvals

- **No production mutation occurred in Phase 1.** This report is the only intended repository write.
- Explicit production-owner approval is required before applying D1 migrations, changing Worker production bindings/configuration/secrets, changing Supabase OAuth settings/redirect URLs, deploying a Worker version, modifying/deleting production identities, or changing production traffic.
- Never print or include secret values in logs or reports. Validate secret presence only.
- Before any production schema mutation, confirm database ID/name, deployed migration history, backup/restore approach, and rollback limits. D1 migrations may not be safely reversible; prefer additive forward repair.
- Do not claim a login flow works without a real authorized end-to-end verification.

## 9. Phase 1 conclusion

**Baseline captured; implementation intentionally not started.** The most consequential confirmed source defect is conflicting duplicate `0002` migrations that add overlapping columns and seed different All-Father principals. Deployed impact is unknown until the actual D1 binding and migration history are inspected read-only. The next phase should begin with migration-runner semantics and authorized deployed-schema evidence, not by immediately editing production migration files.
