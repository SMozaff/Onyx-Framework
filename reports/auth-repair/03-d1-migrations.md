# ONYX Authentication Repair — Phase 3 D1 Migration Sequence Review

**Assessment date:** 2026-10-10  
**Repository:** `SMozaff/Onyx-Framework`  
**Branch at phase start:** `main`  
**Phase-start commit:** `2066216ee08a7222df6bc64f9088a87b3c4791ce` (`Merge pull request #172 from SMozaff/auth-repair/02-auth-contract`)  
**Working branch:** `auth-repair/03-d1-migrations`  
**Decision:** BLOCKED before schema edits pending read-only confirmation of the deployed D1 migration ledger.

## 1. Executive decision

The current branch still contains two distinct migration files with prefix `0002`, duplicate `ALTER TABLE users ADD COLUMN` statements, competing All-Father seed records, and a third migration that adds `supabase_user_id` a second time. The clean-install sequence is therefore not a reliable deterministic foundation as committed.

However, the deployment workflow applies migrations to the remote D1 database automatically when dispatched or when the configured migration branch changes. The repository does not establish which migration filenames have already been recorded in the production D1 ledger. Renaming or deleting historical migrations before that ledger is known could cause a previously applied migration to be treated as new, skipped incorrectly, or applied against a schema that already contains its columns. The prompt explicitly forbids ambiguous migration-history rewrites and production mutation without approval.

**No migration SQL, production database, binding, Worker configuration, identity record, or secret was changed.** I am stopping the implementation portion here rather than guessing at production history. The intended schema can be derived; the missing prerequisite is evidence about which migrations have shipped and which schema is actually present.

## 2. Repository baseline and migration inventory

The phase-start commit was re-fetched from GitHub. The migration directory currently contains exactly these four SQL files:

| File | Blob SHA | Observed responsibility / risk |
|---|---|---|
| `0001_foundation.sql` | `cdfb941a0b5634082ac4cc451eb85fad35685fc5` | Creates `users`, aggregates, events, idempotency, token revocations, push subscriptions, file assets, and audit log. `users` initially has no `email`, `role`, or `supabase_user_id`. |
| `0002_auth_hierarchy.sql` | `14aef117a3a5f7a915e78c865a5eed9459e9144c` | Creates `organizations`, adds `email` and `role`, then seeds `__allfather__` with email `so.muzaff@gmail.com` and organization `__allfather__`. Describes Clerk despite the canonical Supabase/Google contract. |
| `0002_onyx_identity_model.sql` | `69390b56c235573a50d2bb2805368badb523f380` | Also adds `email` and `role`; adds `supabase_user_id`; creates overlapping email index and another organization table definition; seeds `__onyx_allfather__` and `__onyx_root__`. |
| `0003_supabase_identity.sql` | `bf2fad2bff6eff0e2451a1a7fde80d9344c2d978` | Adds `supabase_user_id` again and creates a second, differently named unique index. |

These files were fetched again from `main` during this phase; the duplicate prefix and overlapping DDL are still present. The actual production D1 schema and `_cf` migration ledger were not accessed and remain **unknown**.

## 3. Migration mechanism and environment commands

`deploy/cloudflare/edge-gateway/package.json` defines:

- `npm run d1:local` → `wrangler d1 migrations apply onyx-free-db --local`
- `npm run d1:remote` → `wrangler d1 migrations apply onyx-free-db --remote`
- `npm run check` → `wrangler deploy --dry-run`

The deployment workflow `.github/workflows/deploy-cloudflare-edge.yml` materializes `wrangler.deploy.toml` with a D1 binding named `DB`, the `onyx-free-db` database, and `migrations_dir = "migrations"`; it then runs `npx wrangler d1 migrations apply onyx-free-db --remote --config wrangler.deploy.toml` before deployment. The workflow can run on manual dispatch and pushes to `migration/cloudflare-free-worker`. Therefore these migration files are potentially production-shipped history; repository contents alone cannot prove whether a given migration has been applied.

The current Worker CI `.github/workflows/cloudflare-worker-check.yml` runs `npm install` and `npm run check`. It does not apply the D1 migrations to a clean disposable database, inspect migration ordering, or validate the resulting schema.

## 4. Required authentication schema inferred from the canonical contract and Worker SQL

The following is the required target shape inferred from `docs/SECURITY_AUTHENTICATION_MODEL.md`, `reports/auth-repair/02-auth-contract.md`, Worker SQL, and the migration DDL. This is a target contract, not a claim about production state.

### `users`

- `id TEXT PRIMARY KEY`: canonical ONYX principal ID.
- `username TEXT NOT NULL`: ONYX login username; case-insensitive uniqueness is required by current index/query convention.
- `email TEXT NULL`: optional ONYX login email; do not infer missing values; case-insensitive unique index for non-null values.
- `password_hash TEXT NOT NULL`: ONYX Argon2id hash; All-Father must not gain password authentication by virtue of a seed.
- `organization_id TEXT NOT NULL`: tenant/system scope.
- `role TEXT NOT NULL`: canonical authorization role (`ALL_FATHER`, `ORGANIZATION_ADMIN`, `STAFF`); valid values and legacy backfill semantics must be explicit.
- `is_active INTEGER NOT NULL`: account status.
- `is_admin INTEGER NOT NULL`: legacy compatibility field only, not independent authority.
- `class`, `parent_user_id`, `created_at`, `updated_at`: existing operational/hierarchy fields retained without privilege inference.
- `supabase_user_id TEXT NULL`: explicitly provisioned external Supabase subject; unique for non-null values. Login must never auto-bind or replace this mapping.

### `organizations`

- `id TEXT PRIMARY KEY`, `name TEXT NOT NULL`, `is_active INTEGER NOT NULL`, `created_at INTEGER NOT NULL`, `updated_at INTEGER NOT NULL`.
- Case-insensitive uniqueness for organization names, subject to the existing product contract.
- Every Admin/Staff principal must reference a valid organization; authorization must re-check organization active state.
- A single All-Father principal/system scope is intended. Competing seeded IDs must not be automatically merged or deleted without inspecting and approving existing data.

### Other tables used by Worker code

`token_revocations(token_hash PRIMARY KEY, revoked_at)`, `audit_log`, `aggregates`, `domain_events`, `idempotency`, `push_subscriptions`, and `file_assets` are created by `0001_foundation.sql`. The migration review did not establish a complete inventory of every Worker query against every column, because no disposable D1 migration application or full SQL contract test was run in this phase.

## 5. Concrete mismatches and risks

1. **Duplicate sequence number:** both `0002_auth_hierarchy.sql` and `0002_onyx_identity_model.sql` use prefix `0002`.
2. **Duplicate column DDL:** both `0002` files add `email` and `role`; `0002_onyx_identity_model.sql` and `0003_supabase_identity.sql` both add `supabase_user_id`. Applying overlapping migrations to the same schema can fail.
3. **Competing privileged seed identities:** one migration inserts `__allfather__` / `__allfather__`; another inserts `__onyx_allfather__` / `__onyx_root__`. `INSERT OR IGNORE` does not reconcile those principals, does not guarantee there is only one privileged row, and does not prove which identity production currently uses.
4. **Seeded identity is not explicit external provisioning:** the `0002_onyx_identity_model.sql` seed has `supabase_user_id = NULL`, which is consistent with requiring trusted provisioning, but the competing legacy seed and current app lookup conventions make the actual canonical row ambiguous.
5. **Unconstrained role values:** current DDL does not declare a `CHECK` constraint for canonical roles. A constraint/backfill strategy must account for existing values before enforcement.
6. **No explicit organization foreign key:** the current schema does not enforce the users-to-organizations relationship at the database level. Introducing one requires validating existing rows first and is not safe to guess without schema/data evidence.
7. **Migration CI gap:** `npm run check` is a Worker bundle dry-run, not a D1 migration test. No CI step proves a clean database reaches the target auth schema.
8. **Readiness contract gap:** `/ready` only runs `SELECT 1`; it cannot detect missing authentication tables or columns. Coordinate the final schema contract with Prompt 06; do not invent a second schema version source here.

## 6. Why migration SQL was not changed

There are two materially different safe repair plans depending on the deployed ledger:

- **If none of the conflicting `0002`/`0003` migrations have shipped:** replace the unshipped migration set with one monotonic sequence and add a clean-database migration test. This requires evidence that none of those filenames are already recorded in any supported remote environment.
- **If any conflicting migration has shipped:** preserve its filename/content as historical migration identity and repair forward with a new uniquely numbered migration. But a forward-only migration cannot by itself make a fresh install pass through an earlier migration that fails due to duplicate column creation; the clean-install path must also be versioned/selected safely without making already-applied history ambiguous.

The repository evidence does not establish which case applies. Choosing either plan without the ledger would violate the no-ambiguous-history and no-production-data-loss requirements. A `0004` migration alone is not an adequate fix for the clean-install failure if the earlier files still fail.

## 7. Required owner-approved next action

Before resuming migration edits, authorize a **read-only** inspection of each relevant D1 environment, with no SQL writes:

1. Confirm the database ID/name bound to the production Worker and identify development/preview databases separately.
2. Retrieve the Wrangler/D1 migration list and the migration ledger rows for each environment.
3. Read-only inspect `PRAGMA table_info(users)`, `PRAGMA table_info(organizations)`, `PRAGMA index_list(users)`, `PRAGMA index_list(organizations)`, and counts/IDs/roles/org IDs of privileged users. Do not output password hashes, tokens, or secret values.
4. Confirm whether both All-Father seed IDs exist and whether any real `supabase_user_id` mapping is already provisioned. Do not update or delete either row in this phase.
5. Record backups/restore capability and production migration approval owner before any write is proposed.

Once that evidence exists, choose the migration strategy based on applied history and the supported environments. If production read-only access is unavailable, the owner must explicitly state which migrations are known to have shipped; otherwise keep this phase blocked.

## 8. Proposed validation required before phase acceptance

The next implementation attempt should add an isolated CI test using the repository's official Wrangler migration mechanism against a disposable D1/preview database (not production), plus schema assertions for required columns/indexes and canonical role semantics. The CI check should:

- reject duplicate migration sequence prefixes;
- apply all migrations to a clean disposable database;
- verify the expected tables, columns, indexes, and unique non-null Supabase mapping;
- validate upgrade behavior from each supported previous migration state, including a recorded legacy state if one exists;
- prove no migration grants All-Father role from email domain or arbitrary Google identity;
- detect duplicate/competing privileged seed rows without silently deleting or merging them;
- coordinate a single exported/documented schema contract with Prompt 06 readiness work.

These checks were **not added or executed** because the correct migration rewrite cannot be selected until the deployed history is known. Do not treat this report as a passing migration validation.

## 9. Rollout and rollback policy

- Development: apply only to a disposable/local D1 after the ordered sequence and tests are fixed.
- Preview: apply to a separate preview D1, validate schema and existing-data upgrade behavior, and record migration ledger/output.
- Production: no execution during this phase. Require explicit approval, confirmed database identity, a verified backup/restore plan, preflight schema/ledger capture, and post-migration schema checks.
- Rollback: prefer additive forward-fix migrations. Do not assume D1 DDL can be safely reversed. Never delete/reset/recreate the database to resolve migration errors.
- Identity changes: any migration touching existing All-Father rows, external subject mappings, role values, or user records requires explicit owner review and approval before rollout.

## 10. Phase 3 status

**Inventory and schema reconciliation completed; migration repair blocked pending migration-ledger evidence.** No production migration was executed and no migration SQL was changed. The phase must not be marked as acceptance-complete: deterministic clean-install and supported upgrade tests remain unproven. Resume only after the read-only D1 migration/schema inspection above, then make the smallest history-safe correction and run the migration checks in CI/preview.
