# Backend handover: staff accounts, D1 counters, import (runs in parallel with the Slint migration)

**Status:** Owner allowed this to run in parallel with the migration (2026-11-01). Rules: `docs/product/staff-and-team-rules.md`. Decision: `docs/migrations/slint/decision-log.md`.
**Scope:** Cloudflare Worker (`deploy/cloudflare/edge-gateway`), D1 migrations, the import and single-creation routes, and the Excel creation report. The Slint client (`crates/bins/admin-slint`) is out of scope for this lane.

## Required behaviour (owner decisions)

1. Every staff row in a batch import creates an account and assigns a staff ID. Single admin creation follows the same rule.
2. Staff sign in with their Onyx ID.
3. The system generates a one-time password per account.
4. Staff must change the password at first login.
5. Passwords are never stored, logged, prefilled, or requested in chat. Only a hash is stored.
6. After every import and every single creation, the admin gets an Excel (.xlsx) creation report with: the staff sheet fields, the assigned `onyx_id`, the one-time password, and the account status.
7. ID format and counting: `docs/product/staff-and-team-rules.md` and `docs/design/id-sequences.md`.

## Open decisions (agent must NOT decide these; stop and report)

- **O-1 Hashing:** Worker uses Web Crypto (PBKDF2) or a bundled Argon2id library. The Rust side uses Argon2id. Owner decides.
- **O-2 Where the account store lives:** the identity authority per the migration plan is the Worker and D1. Confirm D1 before any migration. Do not write to the Rust `UserStore`.
- **O-3 Sign-in with Onyx ID:** whether the Onyx ID replaces the username or is an alias. Owner decides.
- **O-4 Forced password change:** how it is enforced (flag on the account, login response that blocks other calls). Owner decides the rule; agent proposes the mechanism.
- **O-5 Blank `onyx_id` in import:** conflicts with "every staff has an account". Owner decides (reject, or create account and ID together).
- **O-6 Existing import:** `POST /api/admin/profiles/import` (CSV/JSON) currently requires an existing account per row. Owner decides whether it is replaced or extended.

## Work the agent may do without deciding O-1 to O-6

- Read the Worker code, D1 migrations, and route-completeness and authorization tests. Document the current sign-in path in `docs/migrations/slint/backend-accounts-current-state.md`.
- Draft D1 migration files for `id_sequences` and `public_ids` (from `docs/design/id-sequences.md`) under a clearly named draft folder, not applied anywhere.
- Write tests that pin the rules above as failing or pending tests, marked as blocked by O-1 to O-6 where relevant.
- Draft the Excel creation report column spec (no code that writes passwords).

## Hard rules

- No production deploys, no merges to main, no D1 writes to production. Open PRs only; the owner merges.
- No real passwords, tokens or account data in files, logs, tests, or PR text. Use generated fixtures only.
- Do not edit the Slint client or the migration docs except the decision log entry for this lane.
- Work on branch `feat/backend-accounts`. Commit with the attribution lines from the session.
- Report back: current-state findings, the list of O-items, the draft files, and the PR number.
