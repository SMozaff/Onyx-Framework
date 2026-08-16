# Task for Manus: Build, test, and verify the ONYX repo (Rust workspace)

## Context

This is a Rust/React local-first team-ops product called ONYX. You've
been connected to the repository. The most recent work (2026-08-16)
added a new feature end-to-end: Todo/Target lists, staff loans, and a
background notification job. Most of it was already built and
live-tested against a real running server in a different sandbox — but
that sandbox had **no PostgreSQL available**, so one specific piece
(raw SQL in the background worker) was never run against a real
database. Your primary job is to close that gap and do a full
build/test pass across the whole workspace.

Read `STATUS_REPORT_2026-08-16.md`, `DECISIONS.md`, and
`IMPLEMENTATION_PLAN_User_Hierarchy.md` (§11 specifically) at the repo
root first — they contain the full history of what was built, what was
verified, and exactly what wasn't. Don't skip this; it'll save you from
re-deriving context that's already written down.

## What's already known to work (don't re-litigate, just confirm)

- `todo-domain` crate (`crates/domains/todo-domain/`): 42 unit tests,
  `cargo test --package todo-domain` should pass clean.
- `security-adapter`/`security-application`: 24 unit tests, should pass
  clean.
- `api-server` HTTP integration for Todo/Target/StaffLoan: was
  live-tested manually (SQLite backend) via curl scripts — full
  create → submit → verify → re-query round trips, verifier
  authorization (D.4), and Team Leader pre-check redaction (D.5). This
  should still work; re-verify it if convenient, but it's not the
  priority.

## What's NOT verified — this is your main job

**`crates/bins/worker/src/staff_loan_scheduler.rs` and the two new job
handlers in `crates/bins/worker/src/job_runner.rs`
(`execute_staff_loan_advance_warning`, `execute_staff_loan_expiry`)
contain raw Postgres SQL that has never been executed against a real
Postgres database.** This includes:

- `jsonb_set(state, '{advance_warning_sent_at}', to_jsonb($3::bigint), true)`
- `state->>'status'`, `state->'window'->>'end_at'` JSON path extraction
- Direct `INSERT INTO domain_events` / `INSERT INTO outbox` / `INSERT INTO aggregates` statements mirroring the existing `execute_timeline_trigger` function's pattern (same file) — that existing function is presumed working (it's older code), use it as your reference for "does this pattern actually work" if the new code doesn't.

### What to do:

1. **Stand up a real Postgres instance** (Docker is fine if available,
   otherwise whatever's easiest in your sandbox).
2. Run the Postgres migrations: `sqlx migrate run` against
   `migrations/postgres/` (see `crates/bins/api-server/src/routes/mod.rs`'s
   `ApiState::new` for the exact migration invocation if you need the
   pattern — it's `sqlx::migrate!("../../../migrations/postgres")`).
3. **Write a real integration test** (or extend an existing one if you
   find a suitable harness — check for `#[sqlx::test]` usage elsewhere
   in the workspace first) that:
   - Inserts a `staff_loan` aggregate row directly (or via the real
     `POST /api/todo/staff-loans` + `staff_loan.ApproveStaffLoan`
     command flow — check `crates/bins/api-server/src/routes/todo_admin.rs`
     and `routes/command.rs` for the real request shapes) with
     `window.end_at` set a few days in the future.
   - Runs `staff_loan_scan_tick_postgres` (already split out for
     exactly this kind of test — see the function's own doc comment in
     `staff_loan_scheduler.rs`) and confirms it enqueues a
     `StaffLoanAdvanceWarning` job when `end_at` falls inside the
     2.5-day lead window, and does NOT re-enqueue it on a second tick
     (the `advance_warning_sent_at` dedup guard).
   - Runs the job execution path (`execute_staff_loan_advance_warning`)
     and confirms: (a) `notification` rows are actually inserted for
     all three parties (staff member, real owner, borrowing manager),
     and (b) the loan's `state` JSON gets `advance_warning_sent_at` set
     afterward — this is the `jsonb_set` call; confirm it actually adds
     a **new** top-level key to a JSON object that didn't have it
     before, since that's the specific thing that was only verified
     against documentation, not by running it.
   - Same for expiry: set `window.end_at` in the past, run the scan +
     `execute_staff_loan_expiry`, confirm the loan's `status` flips to
     `"Expired"` in the stored JSON, confirm a `staff_loan.StaffLoanExpired`
     row lands in `domain_events` and `outbox`, and confirm expiry
     notifications are inserted.
4. **Fix whatever's broken.** If the SQL has bugs, fix them — this code
   was written carefully but genuinely never executed, so treat any
   failure as expected-to-be-found, not surprising. Common failure
   modes to watch for specifically: JSONB path/operator typos, type
   casting issues between `bigint`/`jsonb`/timestamp representations,
   and the `Timestamp` type's actual encoding (`todo_domain::value` —
   it's nanoseconds since epoch as a plain `u64`, confirm the SQL's
   arithmetic matches that unit, not seconds or milliseconds).
5. Once the worker crate's Postgres path is verified, **run the full
   workspace test suite**: `cargo test --workspace` (this may take a
   while and may hit crates unrelated to this feature — that's fine,
   just report what you find). Also run `cargo clippy --workspace --
   -D warnings` if time allows.
6. Also worth a look, lower priority: `admin-shell` and `desktop-shell`
   haven't been touched by this feature and should be unaffected, but a
   `cargo check --workspace` will catch it if something broke silently.

## Environment notes

- Rust toolchain: see `rust-toolchain.toml` at repo root.
- Postgres migrations live in `migrations/postgres/`, SQLite in
  `migrations/sqlite/` — don't confuse them, the worker crate is
  Postgres-only (see `crates/bins/worker/src/main.rs`, it requires
  `DATABASE_URL` to be a Postgres URL and has no SQLite branch at all).
- The workspace is large; if your sandbox has disk constraints, prefer
  targeted `cargo test --package worker` /
  `cargo test --package api-server` runs over `cargo build --workspace`
  where possible, and clean `target/` between major steps if space gets
  tight — the previous build environment ran into exactly this problem
  repeatedly (see `STATUS_REPORT_2026-08-16.md`'s P3 note about it).
- `node_modules` for `admin-shell`/`desktop-shell` UI may not be
  present (deliberately not shipped) — `npm install` in
  `crates/bins/*/ui/` if you need to touch the frontend, but that's out
  of scope for this task.

## Deliverable

A short report: what you verified, what you found broken (if
anything), what you fixed, and the final state of
`cargo test --workspace` / `cargo clippy --workspace -- -D warnings`.
If you fix real bugs in the SQL, please also update
`IMPLEMENTATION_PLAN_User_Hierarchy.md` §11.3 and `DECISIONS.md` with a
dated note — same standard the rest of this project's history is held
to: document what changed and why, in the same file, not a separate
changelog.
