# ONYX Remaining UI Gaps — Completion Report

**Date:** 2026-08-17
**Scope:** User selection for assignment and staff loans; actionable escalation visibility; targeted API and web-ui regression coverage.

## Outcome

All three items in `MANUS_HANDOFF_4_Remaining_UI_Gaps.md` were implemented in the synchronized ONYX workspace. The implementation preserves the existing administrative-user boundary, replaces manual identifiers with a reusable picker, and turns escalation visibility into action-capable work queues rather than read-only lists.

| Work item | Result |
|---|---|
| Non-admin user picker data | Complete: authenticated `GET /api/users` returns active same-organization `{ id, username }` identities only. |
| Todo/Target assignment | Complete: creators may choose self-authored work or assign a list/target to a selected colleague using `ManagerAssigned`. |
| StaffLoan party selection | Complete: staff member, real owner, and borrowing manager are selected through the shared picker; real owner and borrowing manager remain distinct. |
| Escalated Todo/Target work | Complete: an **Escalated to you** filter selects work routed to the current user and opens the established Verify/Reject/Escalate detail actions. |
| Escalated StaffLoan work | Complete: an **Escalated to you** filter exposes loans routed to the current user and enables Approve/Decline on those Requested loans. |
| Documentation | Complete: dated records were added to `DECISIONS.md` and `IMPLEMENTATION_PLAN_User_Hierarchy.md`. |

## Implementation Details

### Least-privilege picker endpoint

The existing `GET /api/admin/users` remains unchanged and admin-only. A new `GET /api/users` route authenticates the bearer token and filters the existing user-store listing to active users in the caller's organization. Its `PickerUserDto` intentionally exposes only `id` and `username`; it does not leak administrator status, manager status, user class, reporting line, or account-activation metadata.

A real HTTP integration test uses a non-admin login and verifies four behaviors: the ordinary user can call the route, the response excludes privileged fields, inactive accounts are excluded, and a user from another organization is excluded. It also confirms that a missing bearer token receives HTTP 401.

### Shared picker and assignment flows

`web-ui/src/components/UserPicker.tsx` is a reusable search-and-select control backed by `/api/users`. It caches results through the existing React Query dependency and filters client-side by username for small directory sets.

`CreateListForm` now provides an explicit **For myself** / **Assign to someone else** choice. The former retains the existing `StaffAuthored` and current-owner payload; the latter sends the picked person as the owner with `ManagerAssigned`. `CreateLoanForm` now uses the same picker three times, eliminating all raw UUID fields while retaining client-side prevention of selecting the same real owner and borrowing manager.

### Actionable escalation views

Todo/Target and Staff Loan projection types now include the optional normalized `escalated_to` field exposed by the existing query surface. Both pages add an **Escalated to you** selector that filters on `escalated_to === currentUser.id`.

The Todo/Target filter preserves the master-detail experience: selecting an escalated list opens `ListDetail`, where an Escalated list is actionable only when the signed-in user is the explicit escalation target. The StaffLoan card uses the same decision-maker rule: the real owner decides while un-escalated; after escalation, the target decides. Client-side visibility remains a UX convenience; the existing server-side authorization guard is the actual authority boundary.

## Regression Coverage

`web-ui/tests/integration/ui_gap_workflows.test.tsx` adds three production-page tests using the repository's existing MSW and React Query test conventions.

| Test | Coverage |
|---|---|
| Shared UserPicker | Search filters people and selection returns the chosen identity id. |
| Todo escalation view | The signed-in user sees only their escalated list and reaches Verify, Reject, and Escalate controls. |
| StaffLoan escalation view | The signed-in user sees only their escalated loan and reaches Approve and Decline controls. |

## Validation Results

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed. |
| `cargo check --package api-server` | Passed. |
| `cargo clippy --package api-server --all-targets -- -D warnings` | Passed. |
| `cargo test --package api-server` | Passed: 29 tests across six integration-test binaries. |
| `npm install` | Completed successfully. |
| `npx tsc -b` | Passed with zero TypeScript errors. |
| `npx vite build` | Passed. |
| `npx vitest run` | Passed: 133 tests passed; 7 live-backend tests skipped as pre-existing. |

The web test run emitted pre-existing jsdom canvas notices from axe-core and React Router v7 future-flag notices. These are warnings only; all test files passed.

## Files Added or Changed

| Area | Key files |
|---|---|
| API | `crates/bins/api-server/src/routes/admin.rs`, `crates/bins/api-server/src/routes/mod.rs`, `crates/bins/api-server/tests/user_hierarchy_admin_routes.rs` |
| Shared UI | `web-ui/src/components/UserPicker.tsx` |
| Assignment UI | `web-ui/src/pages/TodoTargets/CreateListForm.tsx`, `web-ui/src/pages/StaffLoans/CreateLoanForm.tsx` |
| Escalation UI | `web-ui/src/pages/TodoTargets/index.tsx`, `ListDetail.tsx`, `web-ui/src/pages/StaffLoans/index.tsx`, `LoanCard.tsx`, `web-ui/src/types/query.ts` |
| UI tests | `web-ui/tests/integration/ui_gap_workflows.test.tsx` |
| Documentation | `DECISIONS.md`, `IMPLEMENTATION_PLAN_User_Hierarchy.md` |

## Repository State

The requested implementation is present in the local synchronized GitHub checkout at `/home/ubuntu/Onyx-Framwork-github`. No commit or push was made in this pass, because the handoff requested implementation and verification but did not explicitly request publication to GitHub.
