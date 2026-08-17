# Task for Manus: verify recent Todo/Target/StaffLoan escalation work

## Context

Same repo as the earlier handoff (`onyx-repo-full.bundle` / GitHub
`So-Muzaff/Onyx-Framwork`). Since that verification, a lot more was
built on top: Staff Loans UI, a Team Leader authorization fix, the
full Phase E escalation mechanism for Todo/Target lists, staff-loan
escalation, and a field-normalization bug fix. All of it compiles
clean and passes its existing test suite, and most of it was already
live-tested manually in the build sandbox — but a few specific things
were **not** verified due to running low on disk space and wanting to
stop pushing the sandbox further. This handoff is scoped tightly to
exactly those gaps, not a full re-test of everything.

Read `DECISIONS.md` and `IMPLEMENTATION_PLAN_User_Hierarchy.md` (the
newest entries, from today) for full context on each change before
testing — they document what was built, why, and what was already
verified, in detail.

## What's already known to work (skip re-verifying, just be aware)

- `todo-domain`: 53 unit tests pass, clippy clean.
- `api-server`: 25 tests pass, clippy clean, including 3 new tests for
  the Team Leader pre-check authorization fix.
- Todo/Target escalation was live-tested end to end against a real
  running server with a real 3-level tree: routing, D.4 widening
  (escalation target replaces the original verifier), re-escalation.
- Staff-loan escalation was live-tested the same way: request →
  escalate → escalation target approves.
- The field-normalization fix (`staff_user_id` etc. now correctly
  serialize as UUID strings, not raw byte arrays) was live-verified
  with a direct API query.

## What genuinely needs testing — this is the actual ask

### 1. `web-ui` build/test verification for the last 3 commits

### 1. `web-ui`'s "Involving me" filter, against the now-fixed wire shape

No `web-ui` source files changed after commit `f0f6bc3` (that commit's
own build/test run was already clean — 130 tests passed, nothing to
redo there). But commit `04d37bb` (the field-normalization fix)
changes what the **server** sends back, and `web-ui/src/pages/StaffLoans/index.tsx`'s
"Involving me" filter (`loan.staff_user_id === user.id` etc.) was
written against the old, broken wire shape and has never been tested
against the fixed one. Specifically:
- Run `npm install && npx tsc -b && npx vite build && npx vitest run`
  in `web-ui/` — confirm still clean (should be, since no source
  changed, but confirm no regression crept in from the backend fix).
- More importantly: **manually or via a new test, verify the
  "Involving me" filter in `StaffLoansPage` actually works now** —
  create a staff loan via the real API, load the page (or write a
  lightweight integration test that calls `staff_loan.list` and
  checks the returned `staff_user_id`/`real_owner_id`/
  `borrowing_manager_id` are UUID strings matching real user ids, not
  byte arrays). This was verified at the raw API level in the build
  sandbox but never through the actual UI code path.

### 2. Staff-loan approval authorization gap (documented, not fixed)

`crates/bins/api-server/src/routes/command.rs`'s `staff_loan.*`
dispatch arm has a comment (search for `NOTE on authorization here`)
explicitly flagging that `ApproveStaffLoan`/`DeclineStaffLoan`/
`ExtendStaffLoan`/`EndStaffLoanEarly` have **no real server-side
authorization check** — only the domain crate's generic stub. This
means, as of right now, **any authenticated user can approve or
decline any staff loan**, not just the real owner.

This was deliberately left as a flagged gap rather than expanded into
the escalation work's scope. If you have time after the items above,
consider:
- Confirming this gap is real (write a quick live test: two arbitrary
  users, one requests a loan, a completely unrelated third user tries
  to `ApproveStaffLoan` — does it succeed when it shouldn't?).
- If confirmed, fixing it: add a `require_staff_loan_authority`-style
  helper mirroring `require_verifier_authority`/
  `require_team_leader_or_admin` already in the same file, gating each
  command per design doc §2.1's three approval rules:
  - `ApproveStaffLoan`/`DeclineStaffLoan`: only the real owner, or (per
    the newly-added escalation) the loan's `escalated_to` if set —
    `StaffLoan::grants_approval_authority_to()` already exists and
    implements exactly this check; it just isn't called from
    `routes::command` yet.
  - `ExtendStaffLoan`: only the staff member being loaned
    (`staff_user_id`).
  - `EndStaffLoanEarly`: either the real owner or the borrowing
    manager, no approval needed (this one should already work
    correctly by construction, but confirm).
- Add regression tests following the exact pattern of
  `crates/bins/api-server/tests/team_leader_precheck_authorization.rs`
  (3 tests: unauthorized rejected, authorized accepted, escalation
  target accepted after escalation).

## What NOT to do

- Don't re-verify the C.3 Postgres background job — already fully
  verified live against real PostgreSQL in the earlier handoff.
- Don't re-build or re-test `todo-domain`'s unit tests unless something
  in items 1-2 above requires changing that crate — they're already
  green and stable.
- Don't attempt to build an "escalated to you" UI view — that's
  future work, not part of this handoff, and no code for it exists yet
  to test.

## Environment notes

Same as the earlier handoff: `web-ui/` needs `npm install` first (no
`node_modules` shipped). Rust toolchain per `rust-toolchain.toml`. If
disk space gets tight, clean `target/` between major steps rather than
pushing through — this build sandbox hit that limit repeatedly and it
slowed things down more than the cleanup itself cost.

## Deliverable

A short report: what you verified, what passed, what (if anything)
failed or was already broken, and if you fixed the staff-loan
authorization gap, the same documentation standard as before — update
`DECISIONS.md` and `IMPLEMENTATION_PLAN_User_Hierarchy.md` with a
dated note in the same files, not a separate changelog.
