# ONYX Authentication Repair — Phase 3 D1 Migration Sequence Repair

**Assessment date:** 2026-10-10
**Repository:** `SMozaff/Onyx-Framework`
**Base commit:** `2066216ee08a7222df6bc64f9088a87b3c4791ce` (Phase 1–2 report revisions and Worker auth fixes from Phases 1–2 are present uncommitted in the worktree)
**Working tree:** includes Phase 2 changes (`src/index.ts` AUTH-01/AUTH-04 fixes, `test/auth.test.ts`, schema-contract module)
**Prior attempt:** draft PR #173 recorded this phase as blocked; this revision supersedes it. Filenames were never renamed, no production database was touched, and the production rollout remains gated on the owner-approved ledger verification in §10.

## 1. Executive decision

The blocker recorded in draft PR #173 ("cannot fix clean install without knowing the deployed ledger") is **resolved by git-history analysis, without guessing**: the introduction timeline of the migration files proves exactly which ledger states are reachable, and the repair assigns column ownership to the one file whose behavior is consistent in every reachable state. The result:

- **Clean install:** a fresh database reaches the canonical auth schema through the official `wrangler d1 migrations apply` mechanism — verified locally and in CI (§8).
- **Upgrade:** every ledger state reachable through the repository's deployment workflow upgrades cleanly with the pending suffix — verified by tests replaying historical file contents (§7).
- **Production rollout** of the repaired sequence is still **gated** on an owner-authorized read-only ledger/schema verification (§10) because the actual production ledger content cannot be read from this environment. Any state outside the workflow-produced set (manual application, foreign schemas) is detected by that preflight, not assumed.

No migration was executed against production. No production identity, binding, secret, or Worker configuration was changed. No data was deleted.

## 2. Migration inventory (before → after)

| File | Before | After |
|---|---|---|
| `0001_foundation.sql` | Creates `users` (no email/role/supabase), `aggregates`, `domain_events`, `idempotency`, `token_revocations`, `push_subscriptions`, `file_assets`, `audit_log` | **Unchanged** (applied everywhere; content is already canonical for its layer) |
| `0002_auth_hierarchy.sql` | Creates `organizations`, adds `email` + `role`, seeds `__allfather__` | **Unchanged** (applied everywhere a database exists; also correct on clean installs) |
| `0002_onyx_identity_model.sql` | Duplicate `email`/`role` ALTERs, duplicate email index, second `organizations` definition, competing `__onyx_allfather__`/`__onyx_root__` seed | **Repaired:** deliberately **column-free** idempotent bridge (`CREATE TABLE IF NOT EXISTS organizations`, `CREATE INDEX IF NOT EXISTS idx_users_org_role_active`). The competing seed is removed |
| `0003_supabase_identity.sql` | `ALTER ADD supabase_user_id` + second index name `idx_users_supabase_user_id` | **Repaired:** keeps the column ALTER (sole owner) and creates the **canonical** index name `idx_users_supabase_user` |
| `0004_auth_schema_repair.sql` | — | **New forward-only reconciler:** idempotent-only statements — ensures canonical indexes, drops the redundant `idx_users_supabase_user_id`, ensures the canonical inert All-Father seed |

Other migration-adjacent artifacts inventoried:

- **Runner/ledger:** Wrangler `d1 migrations apply`, recording by filename in the D1-managed `d1_migrations` table; pending files applied in lexicographic filename order.
- **Environment commands:** `npm run d1:local` / `npm run d1:remote` (edge-gateway `package.json`); deploy workflow `.github/workflows/deploy-cloudflare-edge.yml` materializes `wrangler.deploy.toml` (`migrations_dir = "migrations"`) and applies `--remote` before each Worker deploy.
- `deploy/cloudflare/edge-gateway/d1/` — **unreferenced duplicate copies** of `0001`/`0002_auth_hierarchy` (byte-identical to `migrations/`). Not referenced by any runner; recommended for removal (owner approval, §10).
- Root `migrations/sqlite/` — timestamped up/down migrations for the **Rust/sqlx** API-server database, a separate system; not D1. Out of scope, noted to prevent conflation.

## 3. Key evidence: introduction timeline (git history)

| Commit | Time (UTC+3:30) | Event |
|---|---|---|
| `b2777b7`/`c4b8d57` | Oct 9 19:04 / 19:12 | `d1/0001` foundation drafted |
| `247f8ca` | Oct 9 20:04 | `d1/0002_auth_hierarchy` |
| `0c105c9`/`fc742e7` | Oct 9 20:06 | tracked `migrations/0001_foundation.sql`, `migrations/0002_auth_hierarchy.sql` |
| `7e56462` | Oct 9 20:29 | `migrations/0003_supabase_identity.sql` introduced — **before any `0002` conflict existed** |
| `f397553` | Oct 10 04:45 | `migrations/0002_onyx_identity_model.sql` introduced — immediately conflicting |

Consequences that determine the repair:

1. A deploy run before Oct 9 20:29 recorded `{0001, 0002a}`.
2. A deploy run between Oct 9 20:29 and Oct 10 04:45 recorded `{0001, 0002a, 0003-original}` — **0003 shipped when `supabase_user_id` did not yet exist anywhere else.**
3. Every run after Oct 10 04:45 attempts `0002b-original`, which **always fails** after `0002a` (`duplicate column name: email`) and is never recorded; the command aborts before reaching later files. `0002b-original` therefore cannot exist in any workflow-produced ledger.
4. The reachable ledger set is exactly: **L1** `{0001}`, **L2** `{0001, 0002a}`, **L3** `{0001, 0002a, 0003-original}` (L1 only if a run landed between the two Oct 9 20:06 commits).

## 4. Canonical auth schema (unchanged target, now enforced)

`users`: `id`, `username`, `email NULL`, `password_hash`, `organization_id`, `is_admin`, `is_active`, `class`, `role`, `parent_user_id`, `supabase_user_id NULL`, `created_at`, `updated_at`.
`organizations`: `id`, `name`, `is_active`, `created_at`, `updated_at`.
Supporting: `token_revocations`, `audit_log`, `aggregates`, `domain_events`, `idempotency`, `push_subscriptions`, `file_assets` (from `0001`).
Canonical indexes: `idx_users_username_lower` (unique), `idx_users_email_lower` (unique partial), `idx_users_role`, **`idx_users_supabase_user`** (unique partial — the external-identity uniqueness constraint), `idx_users_org_role_active`, `idx_organizations_name_lower`.
Single privileged principal: `__allfather__` in org `__allfather__` (matches the Worker's `parent_user_id = "__allfather__"` convention and its `LOWER(username)='allfather'` lookup). The `__onyx_allfather__`/`__onyx_root__` identity model is retired; its seed could never have been recorded through the workflow (§3.2), and a preflight check (§10) confirms no phantom row exists.

**Machine-enforced:** `deploy/cloudflare/edge-gateway/src/schema-contract.ts` exports `AUTH_SCHEMA_CONTRACT` + `missingAuthSchema(db)` (read-only PRAGMA-based). Phase 6 should wire this into `/ready` instead of inventing a second schema source; verify D1 remote acceptance of `PRAGMA table_info`/`index_list` during Phase 6 deployment verification.

## 5. Repair design rules (why this is history-safe)

Wrangler records by filename; content edits to an already-recorded file are inert, and edits to a pending file change only the unshipped suffix. The repair keeps **all four original filenames** and assigns ownership so no reachable state can fail:

1. **Column ownership:** `users.supabase_user_id` is added **only** by `0003` — the oldest identity migration, recorded in every state where the column exists, pending-and-first in every state where it does not. `0002b` is deliberately column-free, so a pending `0002b` can never collide with a column `0003` already delivered (L3), and a pending `0003` delivers the column exactly once (L1/L2/clean).
2. **Idempotent bridge:** pending `0002b` contains only `IF NOT EXISTS` statements on objects owned by `0001`/`0002a` layers.
3. **Forward-only reconciler:** `0004` is idempotent-only, so it converges any reachable state and can be re-applied safely; it also performs the one index-name convergence (`DROP INDEX IF EXISTS idx_users_supabase_user_id` after ensuring the canonical index — an index drop, no data loss).
4. **One principal, no inference:** seeds converge on `__allfather__` via `INSERT OR IGNORE` under the unique username index. No migration writes or infers any existing user's `email`, `supabase_user_id`, `role`, or `organization_id` (verified by test).
5. **No implicit privilege:** the seeded principal is inert — `supabase_user_id` stays `NULL`, the `password_hash` is the documented non-usable placeholder, the Worker rejects password login for `ALL_FATHER`, and (since Phase 2/AUTH-01) Supabase login is denied for unmapped subjects. No migration grants `ALL_FATHER` from a username, email, domain, or Google identity.

## 6. Schema convergence matrix (verified by tests)

| Ledger state | Pending files | Result |
|---|---|---|
| clean (empty DB) | 0001→0002a→0002b→0003→0004 | canonical schema, single index, single inert principal |
| L1 `{0001}` | 0002a, 0002b, 0003, 0004 | same |
| L2 `{0001, 0002a}` | 0002b (bridge no-op), 0003 (adds column+index), 0004 | same |
| L3 `{0001, 0002a, 0003-orig}` | 0002b (bridge no-op), 0004 (ensures canonical index, drops old `idx_users_supabase_user_id`) | same |
| recorded set = all four originals | 0004 | same (only reachable by manual application — preflight §10) |

Pre-existing rows in upgrade states are preserved: emails stay `NULL` (never inferred), roles and organizations untouched.

## 7. Tests

`deploy/cloudflare/edge-gateway/test/migrations.test.ts` (10 cases; real migration SQL against in-memory SQLite, the D1 engine family):
filename uniqueness/order (with the duplicate `0002` prefix preserved as intentional history), clean install, L1/L2/L3 replays using git-extracted **original file contents** as fixtures (`test/fixtures/original-*.sql`), regression proof that `0002b-original` could never apply after `0002a`, data-preservation/no-inference, single-inert-principal assertions, and `missingAuthSchema` gap detection (Phase 6 contract input). `test/auth.test.ts` (9 cases, Phase 2) still passes — 19/19 total.

## 8. Official-mechanism validation and CI

Locally (Node 24, Miniflare): a disposable `wrangler.test.toml` (never committed) with binding `DB` + `migrations_dir = "migrations"`:

- `npx wrangler d1 migrations apply onyx-free-db --local` → all 5 files ✅ (9 commands), exit 0;
- re-run → `No migrations to apply!` (ledger recorded by filename);
- `d1_migrations` lists all five names; schema query returns exactly one `idx_users_supabase_user` and one `users` row (`__allfather__`, role `ALL_FATHER`, `supabase_user_id NULL`).

`.github/workflows/cloudflare-worker-check.yml` now runs, on every Worker PR/push:
1. `npm run check` (bundle dry-run);
2. `npm test` (auth + migration suites);
3. **new:** apply migrations to a disposable local D1 via the official Wrangler runner and assert the ledger recorded ≥5 migrations (`jq`).

No production path is invoked by these steps.

## 9. Rollout strategy (forward-fix only)

- **Development:** `npm run d1:local` (or the CI step) against disposable state; never point at production config.
- **Preview/staging:** apply with the workflow's `wrangler.deploy.toml` shape against a separate database; record `d1_migrations` output and run the preflight (§10.1) before and after.
- **Production:** only after §10 approvals: preflight capture → deploy workflow (applies pending migrations automatically) → post-verify `/ready` (once Phase 6 lands), anonymous-401 check, and the §10.1 readback.
- **Rollback:** SQLite/D1 DDL is not safely reversible. Forward-fix only: correct with a new numbered migration. Never delete/reset/recreate the database to recover from a migration failure. `wrangler d1 migrations apply` stops at the first failing file and records nothing for it, so a failed run leaves the prior state intact.

## 10. Production steps requiring explicit owner approval

1. **Read-only ledger/schema verification** (unblocks final rollout): with owner-authorized, read-only Cloudflare access — confirm the production database ID bound to the Worker; dump `SELECT name FROM d1_migrations`; `PRAGMA table_info(users)`, `PRAGMA index_list(users)`; `SELECT id, username, role, is_active, supabase_user_id FROM users` (never hashes/tokens); confirm which of L1/L2/L3 (or "other") matches, and that no `__onyx_allfather__`/`__onyx_root__` rows exist. If the actual state is outside L1–L3, stop and request a decision rather than applying.
2. **Applying the repaired migrations to production** (via the deploy workflow or `d1:remote`).
3. **Deploying the Worker** built from this tree (includes the Phase 2 AUTH-01 change: Supabase login requires an owner-provisioned `users.supabase_user_id` mapping — provisioning must exist before users may sign in).
4. **Supabase subject provisioning** for the `__allfather__` row (out-of-band, owner-controlled; not a migration).
5. **Cleanup:** deleting the unreferenced `deploy/cloudflare/edge-gateway/d1/` duplicate directory; retiring `scripts/migrate-worker-clerk-to-supabase.py` and the `/api/auth/clerk` alias (compatibility window decision).
6. **Deferred schema decisions** (require data evidence + approval before a future migration): `role` CHECK constraint and users→organizations foreign key (both need table rebuilds or validated backfills; not safe to guess), and any deletion of stray privileged rows.

## 11. Unknowns

- The actual production ledger and schema (readable only via §10.1).
- Whether any database was ever migrated outside the workflow runner (superseded by §10.1 preflight).
- Whether a production D1 database exists at all for this Worker (the live Worker predates the auth-route build; §10.1 also answers this).

## 12. Phase 3 status

**Repair implemented, validated, and documented; production application gated on owner approval.** Migration order is unique-by-filename and deterministic; clean installs pass through the official Wrangler mechanism; all workflow-reachable upgrade states are covered by tests replaying historical content; every Worker auth query matches the final schema (contract-tested); no migration grants privilege implicitly; no production migration was executed. This phase stops here.
