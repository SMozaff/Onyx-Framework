# Task for Manus: close out the remaining UI gaps in web-ui

## Context

Read `DECISIONS.md` and `IMPLEMENTATION_PLAN_User_Hierarchy.md`
(newest entries first) before starting — they document everything
already built and verified this session: the Todo/Target/StaffLoan
backend, escalation mechanism, authorization fixes, and the Todo/
Target/StaffLoan UI screens that already exist in `web-ui`. This
handoff is scoped to exactly what's left in the UI, not a re-test of
anything already verified.

Three things are genuinely unfinished in `web-ui`. Build all three if
time allows; if you have to prioritize, build them in this order —
each is independent, but #1 unblocks #2 for real usefulness.

---

## 1. A user picker (highest priority — this is the actual blocker)

`web-ui/src/pages/TodoTargets/CreateListForm.tsx` only supports
creating a list for yourself (`owner` is always the current user,
`origin` is always `StaffAuthored`). `web-ui/src/pages/StaffLoans/CreateLoanForm.tsx`
requires typing three raw UUIDs by hand for the loan's three parties.
Neither is usable by an actual Manager trying to assign work to a
Staff member, or name real people in a loan request.

**Real blocker to check first, before building the picker UI**: the
only existing endpoint that lists users, `GET /api/admin/users`
(`admin::list_users` in `crates/bins/api-server/src/routes/admin.rs`),
is gated by `require_admin` — only Admins can call it. A picker meant
for ordinary Staff/Managers can't use an Admin-only endpoint. You'll
need one of:
- A new, narrower endpoint (e.g. `GET /api/users` or similar) that any
  authenticated user can call, returning a reduced shape (at minimum
  `id`, `username` — deliberately less than the full `UserDto`, which
  includes `is_admin`/`class`/`parent_user_id`/`is_active` that an
  ordinary user arguably shouldn't need for a picker, though use your
  judgment on what's actually necessary vs. over-restrictive), OR
- Confirming with whoever owns product decisions here whether
  `list_users` should just be opened up more broadly instead.
Don't silently reuse the admin-gated endpoint from non-admin UI code —
that would either break for non-admin users or quietly require every
user to have admin rights, neither of which is right.

**Once the endpoint question is resolved**, build:
- A reusable picker component (search-as-you-type against
  username, or a simple filtered dropdown if the user list is small —
  use your judgment based on what's simplest and still usable) in
  `web-ui/src/components/` (there is no existing one to extend).
- Wire it into `CreateListForm.tsx`: let the creator choose between
  "for myself" (current behavior, `origin: StaffAuthored`) and "assign
  to someone else" (`origin: ManagerAssigned`, `owner` = the picked
  user's id).
- Wire it into `CreateLoanForm.tsx`: replace the three raw-UUID text
  inputs with three pickers (staff member, real owner, borrowing
  manager).

## 2. An "escalated to you" view

Escalation (Todo/Target and staff loans) is fully built and tested on
the backend — `todo_list.status === 'Escalated'` /
`target_list.status === 'Escalated'` / `staff_loan` with a non-null
`escalated_to` all carry an `escalated_to` field naming exactly who
the escalation routes to. Nothing in the UI currently surfaces this —
a Manager has no way to see what's been escalated to them unless they
already know a specific list/loan ID to look up directly.

Build a way to see this. Reasonable approaches, pick whichever fits
best once you've looked at the existing pages:
- A dedicated `/escalations` page listing everything (across Todo,
  Target, and StaffLoan) where `escalated_to === currentUser.id`, or
- A filter/badge added to the existing `TodoTargets` and `StaffLoans`
  pages (similar to `StaffLoansPage`'s existing "Involving me" filter
  — `web-ui/src/pages/StaffLoans/index.tsx` already has a working
  precedent for this exact kind of client-side filter).

Either is fine — use your judgment on which fits the existing app
structure better. Whichever you pick, it needs to actually let the
Manager act from there (verify/reject/re-escalate for lists, approve/
decline for loans) — a read-only list that doesn't link to the real
actions isn't much better than nothing.

## 3. Verify the fix from #1 and #2 don't break anything

Run the full existing verification sequence in `web-ui/`:
```
npm install
npx tsc -b
npx vite build
npx vitest run
```
All should stay clean (currently: `tsc -b` zero errors, build
succeeds, 131 tests pass / 7 skipped). Add real tests for the new
picker component and the escalated-to-you view — follow the existing
test conventions in `web-ui/tests/` rather than inventing a new
pattern.

If you touch `crates/bins/api-server` (for the new user-listing
endpoint), also run:
```
cargo check --package api-server
cargo clippy --package api-server --all-targets -- -D warnings
cargo test --package api-server
```
(currently 28/28 passing) and add a real HTTP-integration test for the
new endpoint following the pattern in
`crates/bins/api-server/tests/team_leader_precheck_authorization.rs`
or `staff_loan_authorization.rs` — boot a real server, hit the real
route, assert on the real response, not a mock.

## What NOT to do

- Don't touch the Docker/e2e suite, the C.3 background job, or
  anything already marked resolved in `DECISIONS.md` — all of that is
  independently verified and out of scope here.
- Don't add admin-gating to the new user-listing endpoint "to be
  safe" — that defeats the purpose; ordinary Staff/Managers need to
  use this picker too.
- Don't build a full org-chart browser or anything beyond a working
  picker — a simple, correct search/select is the goal, not a new
  major feature.

## Deliverable

Same standard as before: a short report on what you built, what you
tested and how, and a dated entry in `DECISIONS.md` and
`IMPLEMENTATION_PLAN_User_Hierarchy.md` describing the change, in the
same files, not a separate changelog.
