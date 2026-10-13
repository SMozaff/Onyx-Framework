# ONYX Authentication Repair — Phase 4: All-Father Identity Definition and Safe Provisioning

**Assessment date:** 2026-10-11
**Repository:** `SMozaff/Onyx-Framework`
**Branch:** `auth-repair/implementation-01-03-resolved`
**Phase-start commit:** `08b4738` (Phase 3 migration repair), based on `15b05a4` (`main` tip)
**Prerequisites read:** `01-baseline.md`, `02-auth-contract.md`, `03-d1-migrations.md`, `docs/SECURITY_AUTHENTICATION_MODEL.md`, `docs/Onyx Auth system.md`
**Decision:** Implementation complete and verified locally. **Production provisioning is PENDING owner action.** No production database, principal, secret, binding, Worker variable, or redirect was changed.

## 0. Correction carried into this phase

The Phase 3 report committed at the phase-start commit was the *superseded* "blocked" draft, not the full repair report: the earlier rebase resolved the add/add conflict in `reports/auth-repair/03-d1-migrations.md` toward `main`'s draft. The full report was recovered from commit `98aa7ed` and restored to the working tree. The migration repair it describes was already committed and is unchanged; only the narrative document was wrong.

## 1. The canonical All-Father identity model

One authoritative ONYX identity model, matching `docs/SECURITY_AUTHENTICATION_MODEL.md` and the Phase 2 contract (B) and Phase 3 schema:

| Element | Canonical value / source of truth | Notes |
|---|---|---|
| ONYX principal ID | `users.id = "__allfather__"` | Stable; matches the Worker's `parent_user_id` convention |
| Login username | `users.username = "allfather"` | **Mutable profile attribute** — a lookup hint, never an authorization input |
| ONYX role | `users.role = "ALL_FATHER"` | Explicit D1 value; the only thing that makes the Worker treat the principal as All-Father |
| ONYX admin flag | `users.is_admin = 1` | Additional Worker gate; not independent authority |
| Status | `users.is_active = 1` | Revalidated on login, refresh, and every protected request |
| System scope | `users.organization_id = "__allfather__"` | Reserved system scope, seeded by `0002_auth_hierarchy.sql` |
| Password | non-usable `$argon2id$…AAAA…` placeholder | The Worker rejects password login for `ALL_FATHER`; no real credential exists |
| External identity binding | `users.supabase_user_id = <Supabase Auth user UUID>` | **Stable identity attribute.** Created `NULL` by the migrations; written only by the owner-run provisioning command |
| External profile | Supabase-verified primary email (+ `email_confirmed_at`) | **Mutable profile attribute**; verified at login and compared to the configured designated All-Father email |
| Mutable D1 profile | `users.email`, `users.class`, `users.created_at/updated_at` | Not authorization inputs |

**Stable vs mutable:** authorization rests on `id`, the explicit `role`, `is_active`, `is_admin`, `organization_id`, and the immutable `supabase_user_id` subject. Username, email, `class`, and timestamps are profile data that may change without altering privilege, and no client-supplied value is ever authoritative.

**Uniqueness:** `idx_users_supabase_user` (unique, partial on non-NULL) is the database-level external-identity uniqueness constraint, and the single canonical principal is enforced by `idx_users_username_lower` plus the provisioning preflight.

## 2. Narrow code changes in this phase

1. **New owner-only provisioning tool** — `deploy/cloudflare/edge-gateway/scripts/provision-allfather.mjs` (npm alias `provision:allfather`). Details in §3–§5.
2. **Removed one residual implicit-privilege path** — in `src/index.ts`, `roleForUser()` no longer returns `ALL_FATHER` because `users.username` is the literal `allfather`. Privilege now comes only from the explicit `users.role` value; a row with an empty role degrades to `STAFF`/`ORGANIZATION_ADMIN` (fail-closed, never an implicit grant). This directly satisfies the Phase 4 criterion "privileged identity mapping is explicit and unique".
3. **Test-fixture correction** — `test/auth.test.ts` used the retired competing seed IDs (`__onyx_allfather__` / `__onyx_root__`); updated to the canonical `__allfather__` identity model.
4. **Documentation** — Worker `README.md` gained an "All-Father subject provisioning (owner-only, out-of-band)" section with the full runbook; `docs/SECURITY_AUTHENTICATION_MODEL.md` gained a Phase 4 reconciliation note.

No migration, deployment configuration, Worker variable, secret, redirect allowlist, or production record was touched.

## 3. Why a provisioning command exists

The migrations intentionally seed an **inert** principal (`supabase_user_id = NULL`, unusable password hash), and since Phase 2 the Worker denies every unmapped Supabase subject (`403 ALLFATHER_NOT_PROVISIONED`, zero writes). Something must therefore write the mapping out-of-band. That "something" must not be login, a migration, a client, or a Supabase admin API call, and it must be auditable and reversible. The command is the smallest mechanism that satisfies all four.

**No secrets are needed or read.** The tool only needs the Supabase user UUID (copied by the owner from the provider console) and normal Wrangler credentials for D1. It never calls `SUPABASE_SECRET_KEY`, never reads `password_hash`, and never logs tokens.

## 4. Command design (fail-closed)

Interface (`node scripts/provision-allfather.mjs`):

| Flag | Purpose |
|---|---|
| `--environment local\|preview\|production` | Required. Selects `--local` vs `--remote`; production additionally requires approval |
| `--action provision\|revoke` | Provision (default) or revoke the subject mapping |
| `--supabase-user-id <uuid>` | Supabase Auth user UUID; must match the UUID pattern or the command refuses **before any SQL** |
| `--activate` | Required to flip an inactive principal to active |
| `--rebind` | Required to replace an existing subject (approved identity replacement) |
| `--approve-production <value>` | Required for production; must restate the subject (provision) or `__allfather__` (revoke) |
| `--expect-database <name>` | Refuse if it does not match `--database` (database-identity pre-check) |
| `--config <path>` | Wrangler config; defaults to `wrangler.deploy.toml` for production |
| `--dry-run`, `--json`, `--operator <label>` | Plan only / machine output / audit label |

Refusal codes (never a partial write):

| Code | Trigger |
|---|---|
| `INVALID_ACTION` / `INVALID_ENVIRONMENT` / `INVALID_SUBJECT` | Bad input, including any email address supplied as a subject |
| `PRODUCTION_APPROVAL_REQUIRED` | Production without an approval value restating the subject/principal |
| `DATABASE_MISMATCH` | `--expect-database` ≠ `--database` |
| `AUTH_SCHEMA_MISSING` | `users` lacks a required column (migrations not applied) |
| `PRINCIPAL_MISSING` | The canonical `__allfather__` row does not exist — **the tool never creates a principal** |
| `PRINCIPAL_AMBIGUOUS` | Zero or more than one `ALL_FATHER`/`allfather` row — no merge, no delete |
| `PRINCIPAL_ROLE_NOT_EXPLICIT` | `users.role` is not the explicit `ALL_FATHER` value |
| `PRINCIPAL_NOT_ADMIN` | `users.is_admin` ≠ 1 |
| `PRINCIPAL_INACTIVE` | Inactive and `--activate` absent |
| `SUBJECT_BOUND_TO_OTHER_PRINCIPAL` | The subject is already mapped to a different ONYX principal |
| `REBIND_REQUIRES_APPROVAL` | The principal already has a different subject and `--rebind` is absent |
| `CONCURRENT_MODIFICATION` | The postcondition re-read does not match the plan (no audit record is written) |

Behaviour:

- **Idempotent:** re-running with the same subject is a no-op → `already-provisioned`.
- **Guarded write:** `UPDATE … WHERE id = '__allfather__' AND (supabase_user_id IS NULL OR supabase_user_id = <preflight value>)`.
- **Verified postcondition:** after writing, the row is re-read and compared to the intended subject (see §6.3 — Wrangler's `d1 execute --json` does not report affected-row counts, so the guard is proven by re-reading). The audit row is written **only after** the postcondition holds, so the audit log can never misstate an outcome.
- **Audited:** `audit_log` gains one `allfather.provision` or `allfather.revoke` row with `{target, environment, action, previous_subject, new_subject, activated, rebind, operator, correlation_id, created_at}` — no credential material, no token, no hash.
- **Reversible:** `--action revoke` clears the mapping (audited, idempotent → `already-revoked`), which is the break-glass first step before rebinding a replacement identity.

## 5. Threat model: what cannot grant All-Father

| Attempted path | Outcome |
|---|---|
| Any Google account signs in via Supabase | Denied: subject must equal the provisioned `supabase_user_id` |
| A Google account with the designated email but an unrecognized subject | Denied: mapping mismatch (`ALLFATHER_NOT_PROVISIONED`) |
| A token for the wrong issuer/project/audience, expired, or bad signature | Denied (`INVALID_SUPABASE_TOKEN` / `ALLFATHER_REQUIRED`) |
| A subject already bound to another ONYX principal | Provisioning refuses (`SUBJECT_BOUND_TO_OTHER_PRINCIPAL`) |
| Creating a second `ALL_FATHER` principal or a `username=allfather` row | Refused (`PRINCIPAL_AMBIGUOUS`) or explicitly impossible at provisioning; an empty-role row is no longer promoted by the username |
| A client-supplied `role`/`is_admin`/`allfather` flag | Ignored — authorization is resolved from D1 server-side |
| Reaching production accidentally | `--environment production` + `--approve-production <subject>` + `--expect-database` are all required; `--local` is never implicit |
| A secret leaking from this work | The tool requires no provider secret and reads no `password_hash`; test assertions reject credential-shaped audit fields |

## 6. Verification (all executed locally, no network to production)

### 6.1 Unit/integration tests — 37 passing

`npx vitest run` (the repository's existing vitest convention; CI runs the same command):

- `test/provision-allfather.test.ts` — **17 new tests** on a real migrated in-memory SQLite database: inert-seed precondition, provision + exactly one audit record + no credential-shaped fields, idempotence, dry-run writes nothing, subject double-bind refusal, ambiguous-principal refusal, principal-missing refusal (nothing created), implicit-role refusal, inactive refusal then `--activate`, `--rebind` gating with `previous_subject` recorded, missing-schema refusal, input/environment guards, production approval, database mismatch, SQL-literal escaping, and revoke (+ `already-revoked`).
- `test/auth.test.ts` — 10 tests, including a new one proving that an empty-role `username=allfather` row is **not** treated as `ALL_FATHER`; verified to fail when the implicit username inference is re-introduced (401 instead of 200).
- `test/migrations.test.ts` — 10 unchanged Phase 3 tests.

### 6.2 Worker bundle

`npm run check` (`wrangler deploy --dry-run`) → exit 0, ~77 KiB upload, no bindings resolved locally as expected.

### 6.3 End-to-end through the real operator interface (Wrangler + local D1)

A disposable local D1 was created with the official runner (`wrangler d1 migrations apply … --local`), then driven with the actual CLI:

1. clean preflight → `supabase_user_id` is `null`;
2. `--supabase-user-id so.muzaff@gmail.com` → refused `INVALID_SUBJECT`, no SQL sent;
3. provision → `status: provisioned`, mapping written, `SELECT COUNT(*) FROM audit_log` = **1**;
4. idempotent re-run → `already-provisioned`, 0 writes, still 1 audit row;
5. `--expect-database other-db` → refused `DATABASE_MISMATCH`;
6. rebind without `--rebind` → refused; with `--rebind` → provisioned; revoke → mapping cleared, `allfather.revoke` audited.

**Bug found and fixed by this test:** the first CLI run reported `CONCURRENT_MODIFICATION` while the row had in fact been updated and no audit row existed, because Wrangler's `d1 execute --json` returns `{results, success, meta:{duration}}` with no affected-row count. The guard was reworked into an explicit postcondition re-read (§4) and the CLI re-verified; the regression is now covered by the `CONCURRENT_MODIFICATION` path plus the end-to-end check above. This is exactly the class of defect that unit tests against a `node:sqlite` adapter would not have caught.

## 7. Runbook

The full operator runbook lives in `deploy/cloudflare/edge-gateway/README.md` (§ "All-Father subject provisioning"). Summary of the approved sequence:

1. **Read-only preflight** — `SELECT id, username, role, is_active, is_admin, supabase_user_id FROM users` (never hashes/tokens). Confirm exactly one `ALL_FATHER` row, id `__allfather__`, `supabase_user_id IS NULL`. If not, stop.
2. **Obtain the Supabase user UUID** from the owner's Supabase dashboard (Authentication → Users) for the Google identity — never from the email address.
3. **Dry run** — `--dry-run --json` and confirm the plan.
4. **Apply** — requires the migrations and the Worker to be deployed first (login stays denied until the mapping exists):
   `… --environment production --config wrangler.deploy.toml --database onyx-free-db --expect-database onyx-free-db --supabase-user-id <UUID> --approve-production <UUID> --operator "<owner>"`.
5. **Verify** — `status: provisioned`; the readback returns the UUID; one `allfather.provision` audit row; the Google sign-in then returns an ONYX session instead of `403 ALLFATHER_NOT_PROVISIONED`.
6. **Break-glass replacement** — `--action revoke --approve-production __allfather__`, then `--supabase-user-id <NEW_UUID> --rebind --approve-production <NEW_UUID>`; both audited.

**Operational note:** local Wrangler D1 state is keyed by the config's `database_id`, not the database name (observed during §6.3). Use a distinct `database_id` per disposable environment, and always pass `--expect-database` before a `--remote` run.

## 8. Production steps requiring explicit owner approval

1. Read-only production D1 preflight per `03-d1-migrations.md` §10.1 (ledger, `PRAGMA table_info(users)`/`index_list(users)`, principal rows, `supabase_user_id`).
2. Applying the repaired migrations to production.
3. Deploying the Worker built from this tree.
4. **Running this provisioning command** to write the production `supabase_user_id` (this is the step that enables All-Father sign-in).
5. `--action revoke` / `--rebind` for any future identity replacement.

**No production principal, database, secret, binding, or Worker variable was created, activated, promoted, rebound, or changed in this phase. Production provisioning remains pending owner action.**

## 9. Unresolved decisions for the owner

1. **Email gate vs subject mapping.** `authSupabaseAllFather` currently requires both the designated verified email (`ONYX_ALLFATHER_EMAIL`, default `so.muzaff@gmail.com`) *and* the provisioned subject mapping. Recommend keeping the subject mapping as the authoritative binding and treating the email as defense-in-depth, or deciding explicitly to drop the email gate (provider emails are mutable).
2. **Where `ONYX_ALLFATHER_EMAIL` is configured for each environment** (the deploy workflow and Worker variables), and whether the hard-coded default in `src/index.ts` should remain.
3. **Whether the seeded principal should carry `users.email` at all** — it is profile data and is not used for authorization, but it currently duplicates the designated-email configuration.
4. **Audit retention/ownership** for the `allfather.provision`/`allfather.revoke` rows and the `__owner__:<environment>` actor marker.
5. **Retiring legacy paths** that still describe auto-binding: `obsolete one-time migration script` (contains the old `UPDATE users SET supabase_user_id=…` login rewrite) and the `/api/auth/supabase` alias.
6. **Verification of the real Supabase project**: confirm the Google identity's actual Supabase user UUID and that the project's issuer/JWKS/audience policy matches the Worker before provisioning production.

## 10. Phase 4 status

**All-Father identity is explicit, unique, and fail-closed; a safe, idempotent, auditable, environment-aware provisioning command is implemented and verified; no production principal was changed.** Tests: 37/37 passing locally; Worker bundle dry-run passes; the CLI was exercised end-to-end against a real local D1 through the official Wrangler migration/execute interface, which also surfaced and fixed a real defect. Production provisioning is pending owner approval. The agent stops here.
