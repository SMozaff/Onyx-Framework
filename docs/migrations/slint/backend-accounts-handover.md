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


## Dated decision-status correction — 2026-11-10

The O-1–O-6 list above is preserved as the original handover-time instruction snapshot. It is **not the current decision register**. The coordinator recorded O-1–O-6 on 2026-11-01 under the owner's standing “decide and document” authority. The current statuses are RECORDED delegated decisions, not unresolved owner questions:

- O-1: Argon2id via existing `argon2-wasm-edge`.
- O-2: D1 is authoritative; no Rust `UserStore` writes for this lane.
- O-3: Onyx ID primary login; username/email remain aliases during transition.
- O-4: `must_change_password`; only password change and logout are allowed while set; other session routes return `PASSWORD_CHANGE_REQUIRED` (409).
- O-5: generate a new Onyx ID for each import row; ignore and report a supplied ID.
- O-6: extend the existing import endpoint for XLSX and account creation per row.

The original “agent must NOT decide” instruction applied before the delegated decisions were recorded. It remains useful historical context but must not trigger the same six questions again. The owner may override a delegated decision through a new, traceable decision-log entry.

The owner allowed this backend lane to run in parallel with the Slint migration on 2026-11-01. Continue to respect the existing scope and safety rules: no production deployment, production D1 writes, credential disclosure or unapproved Slint client changes. Verify implementation against the current Worker source and tests; a decision or API-contract description is not by itself deployment evidence.

See [decision log](decision-log.md), [current-state baseline](backend-accounts-current-state.md), and [Worker API contract](../../production/api-contract.md).


## Master record reference

Consult the [Slint Migration Master Record](MASTER-MIGRATION-RECORD.md) before continuing this lane or asking the owner to decide an item. The master and decision log contain current status; this file remains the lane-specific handover.

## Source implementation status — 2026-11-10

The “Work the agent may do” and O-1–O-6 open-decision sections above are historical instructions from before the backend lane's decisions and implementation. The current Worker source on the audited main baseline contains O-1–O-6 behavior, and `test/backend-accounts.test.ts` contains corresponding coverage. See the dated reconciliation in [backend current state](backend-accounts-current-state.md), [creation report spec](backend-accounts-creation-report-spec.md), and the [master record](MASTER-MIGRATION-RECORD.md).

The source implementation and test file do not prove a passing test run on the current revision, production D1 migration application, or a deployed Worker. Keep the lane's original safety constraints: no production D1 writes/deploys without explicit authorization; verify current source and CI before claiming tested/deployed.
