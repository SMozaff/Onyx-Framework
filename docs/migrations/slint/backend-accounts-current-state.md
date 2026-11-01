# Backend accounts: current state of sign-in and account creation

**Status:** Current-state findings, 2026-11-01. Read-only review of `deploy/cloudflare/edge-gateway` on branch `feat/backend-accounts`. No code behaviour was changed. Nothing was run against production D1.
**Lane:** `docs/migrations/slint/backend-accounts-handover.md`. Open items O-1 to O-6 are listed at the end and are **not decided** here.

## 1. Sign-in path today

Route: `POST /api/auth/login` (`src/index.ts`, `authLogin`; route wired in `fetch`; diagnostics wrapper `instrumentLoginRequest`).

1. **Body parse.** Non-JSON or non-object body gives `400 INVALID_LOGIN_REQUEST`. A `username`, `email` or `password` that is not a string also gives `400`.
2. **Identifier.** `username` if present, otherwise `email`, trimmed. An empty identifier or empty password gives `401 INVALID_CREDENTIALS`.
3. **Account lookup.** One D1 query on `users` joined to an active `organizations` row, with `is_active = 1`, matching `LOWER(username) = LOWER(?)` **or** `LOWER(email) = LOWER(?)`. So **login accepts either username or email today.** There is no Onyx ID column or lookup.
4. **Role and reserved identity.** The stored `role` must be one of `ONYX_ROLES`. The reserved All-Father identity must match the canonical principal exactly (`isCanonicalAllFather`).
5. **Password check.** The stored `password_hash` must start with `$argon2id$`, and is checked with `argon2Verify` from `argon2-wasm-edge`. A missing user, inactive user, bad role, bad hash or wrong password all return the same `401 INVALID_CREDENTIALS`.
6. **Client type.** `client_type` must be valid, otherwise `400 INVALID_CLIENT_TYPE`. This check runs after the password check.
7. **Session.** `issuePair` signs an access token (1 hour) and a refresh token (7 days) with HMAC-SHA256 using `ONYX_JWT_SECRET`. Claims: `sub`, `username`, `organization_id`, `is_admin`, `class`, `role`, `client_type`, `sid`, `typ`. The response includes a `user` object with no password fields.
8. **Each protected request** (`requireSession`) re-reads the user row and checks: the account is active, the organization is active, the role is valid, username, organization, role and admin flag still match the token, the token is not revoked (`token_revocations`), and the session is not revoked (`user_session_revocations`). Refresh tokens are rejected as bearer tokens.

**Password hashing.** `hashPassword` uses argon2id with 64 MiB memory, 3 iterations, parallelism 1, a 16-byte random salt and a 32-byte hash. Used by `createUser` and `setUserPassword`.

**Logging.** Login failures and successes go through `makeWorkerLoginDiagnosticEvent`, which logs allowlisted fields only. Tests in `test/auth.test.ts` assert that credential material is not logged.

**Test coverage of this path.** `auth.test.ts`, `auth-phase1-regressions.test.ts`, `route-completeness.test.ts`. The unit tests replace argon2 with `test/argon2-wasm-edge.mock.ts`, which uses `hash-wasm` under Node. The real `argon2-wasm-edge` module runs only in the separate Wrangler smoke workflow, which was not run for this review. Baseline before any change: 13 test files, 191 tests, all passing.

## 2. Account and profile creation today

| Path | What it does | Relevant facts |
|---|---|---|
| `POST /api/admin/users` (`createUser`, STAFF) | Creates a `users` row with `role = STAFF`, `parent_user_id` = the calling admin. | Caller must be `ORGANIZATION_ADMIN`. Body keys must be exactly `username`, `password`, `email`, `class`. **Password minimum 8 characters.** **Email is required** (`EMAIL_REQUIRED`). The admin supplies the plain password in the request. Writes an `audit_log` row `staff.create` whose details include the email. Response has no password. |
| `POST /api/admin/users/:id/password` (`setUserPassword`) | Admin sets a staff member's password. | **Password minimum 12 characters.** Staff only. Writes `audit_log` `user.password.reset` without the password. |
| `POST /api/admin/profiles/import` (`importProfiles`) | Batch import (CSV or JSON). Creates or updates **staff profiles**, not accounts. | Caller must be `ORGANIZATION_ADMIN`. Max 500 rows and 2,000,000 characters. Each row needs `user_id` as a UUID of an **existing** active user in the same organization, otherwise `PROFILE_OWNER_NOT_FOUND` for that row. Required non-empty: `full_name`, `contact_email`, `contact_phone`, `job_title`, `department`. Each row is processed on its own; the response gives `created`, `updated`, `failed` and per-row results. No account is created. |
| `POST /api/admin/profiles` (`upsertProfile`) | Single profile upsert. Used by the import per row. | Same existing-account requirement. |

**Schema facts** (`migrations/`, the folder Wrangler reads through `migrations_dir`; latest file `0008_outbox_ingest.sql`):

- `users`: `id`, `username`, `organization_id`, `password_hash`, `is_admin`, `is_active`, `class`, `parent_user_id`, `created_at`, `updated_at` (0001). `email` and `role` added in 0002. `supabase_user_id` added in 0003.
- There is **no `onyx_id` column** and **no forced-password-change flag** on `users`.
- `aggregates` with `aggregate_type = 'staff_profile'` holds profiles, keyed by the user's UUID through `owner_id` in the state JSON.
- `id_sequences` and `public_ids` do not exist yet (see the draft under `deploy/cloudflare/edge-gateway/drafts/backend-accounts/`).
- `d1/0001_foundation.sql` and `d1/0002_auth_hierarchy.sql` are a second copy of early migrations. `wrangler.dev.toml` does not reference that folder. Not checked further.

**Seeded All-Father row.** `migrations/0004_auth_schema_repair.sql` seeds an inert `__allfather__` principal, with a non-usable password hash. It contains a real email address. That address is not copied into this document or any draft.

## 3. Gaps against the owner rules (handover, "Required behaviour")

| Rule | Current state |
|---|---|
| 1. Every staff row in import creates an account and assigns a staff ID | Not done. Import creates profiles only and requires an existing account. |
| 1. Single admin creation follows the same rule | Partly. Creates an account, but no staff ID is assigned. |
| 2. Staff sign in with Onyx ID | Not done. Login matches username or email. No Onyx ID column. |
| 3. System generates one-time password | Not done. Admin supplies the password. |
| 4. Forced change at first login | Not done. No flag, no enforcement. |
| 5. Passwords not stored, logged, prefilled or requested in chat | Hash-only storage and no password logging already hold for the Worker. Admin-supplied plain password is a current input (`createUser`, `setUserPassword`). |
| 6. Excel creation report with password | Not done. No report exists. Draft column spec: `docs/migrations/slint/backend-accounts-creation-report-spec.md`. |
| 7. IDs per `id-sequences.md` | Not done. Draft migration only. |

## 4. Other findings (not in the O-list, for owner awareness)

- **Password minimum mismatch.** Creation accepts 8 characters. Reset requires 12. One rule should be chosen. Not decided here.
- **Email required for account creation, optional in the staff sheet.** `createUser` requires `email`. The sheet's `contact_email` is optional. An import that creates accounts would need an email for every row, or a rule for rows without one.
- **Email in audit log.** `staff.create` stores the email in `audit_log.details`.
- **Decision-log conflict.** `docs/migrations/slint/decision-log.md` has a later entry "Postponed: staff account import and creation report" (owner decision, 2026-11-01). The handover says the owner allowed this lane to run in parallel on the same date. The two need owner confirmation.
- **Rust side not read.** The Rust `UserStore` and `id_generator.rs` were not reviewed in this pass.

## 5. Open items (not decided by the agent)

**O-1 Hashing.** Found: the Worker already hashes with argon2id through `argon2-wasm-edge` (creation and reset), and verifies `$argon2id$` hashes only. Unit tests exercise a `hash-wasm` mock. The real wasm path is covered only by the smoke workflow. The Web Crypto PBKDF2 option is not present in the code. Owner decides.

**O-2 Account store.** Found: the login, session re-check and account routes read and write D1 `users` only, as far as the Worker code in this review shows. The Rust `UserStore` was not reviewed, so whether any other component writes to it is unknown. Migration `0009` is drafted only, not applied. Owner confirms D1 before any migration is applied.

**O-3 Onyx ID sign-in.** Found: login uses username or email. No Onyx ID column exists. Replacing the username would change the unique-username index and the username claim in the JWT. An alias would need a second unique column and a second lookup. Owner decides.

**O-4 Forced password change.** Found: no flag exists. Existing per-request checks in `requireSession` (role, org, revocation) would be the place to enforce it. Agent proposal only, not implemented: a `password_change_required` column on `users`, returned in the login response, and `requireSession` rejecting every route except the password-change route while the flag is set. Owner decides the rule.

**O-5 Blank `onyx_id` in import.** Found: the current import rejects a row whose `user_id` is not a UUID, with `INVALID_PROFILE_ROW`, before any lookup. Under the new rule a blank ID would need either rejection (same as today) or account and ID created together. Owner decides.

**O-6 Existing import.** Found: `POST /api/admin/profiles/import` is profile-only, and each row needs an existing account. Owner decides whether it is replaced by the account-creating import or extended.

## 6. Verification

- `npx vitest run` in `deploy/cloudflare/edge-gateway`: baseline 191 passing before this lane's changes. See the PR for the post-change run.
- No production D1 access. No deploy. No merge.
