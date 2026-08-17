# Native Desktop Notifications — Implementation Report

**Date:** 2026-08-17

**Scope:** `desktop-shell` and the shared Rust composition path required to make its notification inbox functional.

**Prepared by:** Manus AI

## Outcome

The desktop shell now has a native **Notifications** inbox at
`/notifications`. It lists notifications addressed to the current local
user, permits acknowledgement of pending items, and refreshes when the
existing local event bridge emits a notification event. The implementation
uses the established Tauri IPC and `client-composition` path only; it adds
no browser HTTP client, polling loop, or parallel push system.

| Capability | Delivered behavior |
|---|---|
| Shared aggregate | New `notification-domain` crate owns the aggregate, commands, events, errors, and aggregate tests. |
| Native local inbox | `ListNotifications` reads the tenant- and recipient-scoped SQLite replica through `client-composition`. |
| Acknowledgement | `Acknowledge` dispatches through the real command registry and persists state locally. |
| Live refresh | The existing outbox → `SyncAgent` → `EventBus` → `subscribe_events` → `onyx:event` route refreshes the page. |
| Desktop navigation | A routed page and sidebar navigation item expose the inbox. |

## Implementation Details

Notification domain types had previously lived inside an API route module.
They now reside in `crates/domains/notification-domain`, with `api-server`
re-exporting the types for compatibility. This permits `client-composition`
to use the identical aggregate semantics rather than recreating an
API-specific or desktop-specific notification model.

`AppState` now constructs a local notification repository, registers
`NotificationDecisionHandler` for `Acknowledge`, and exposes
`GetNotification` plus `ListNotifications`. The list handler scopes its
SQLite projection query by organization and recipient. The resulting path
is the same real registry-and-repository path used by other desktop
aggregates.

A delivery defect was corrected in `api-server::command_handler`: committed
events are now registered in the local outbox. The existing outbox pump can
therefore publish them through `EventBus`. The desktop shell starts the
existing `SyncAgent` at application initialization, and the new page calls
the existing `subscribe_events` IPC command and listens for `onyx:event`.
No extra notification transport was introduced.

| Main file | Change |
|---|---|
| `crates/domains/notification-domain/src/lib.rs` | Shared notification aggregate and two unit tests. |
| `client-composition/src/app_state.rs` | Repository, command, and query registration. |
| `client-composition/src/query_registry.rs` | Recipient- and tenant-scoped `ListNotificationsHandler`. |
| `client-composition/src/handlers/decision_handler.rs` | `NotificationDecisionHandler` for acknowledgement. |
| `api-server/src/command_handler.rs` | Registers committed events in the local outbox. |
| `desktop-shell/src/lib.rs` | Starts the existing `SyncAgent`/outbox pump. |
| `desktop-shell/ui/src/pages/Notifications.tsx` | Native inbox, acknowledgement action, and event-driven refresh. |

## Verification

The Rust checks below completed successfully. The real SQLite integration
test is intentionally not a mock: it verifies recipient filtering,
acknowledgement persistence, and event-bus delivery through the registered
composition root.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| Focused `cargo check` for `notification-domain`, `client-composition`, `desktop-shell`, and `api-server` | Passed |
| Focused `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test --package notification-domain --package client-composition` | Passed: 2 notification-domain and 31 client-composition tests |
| `app_state_wires_notification_inbox_acknowledgement_and_events` | Passed against real SQLite and the actual event bus |
| `npx tsc -b` in `desktop-shell/ui` | Passed with zero errors |
| `npx vite build` in `desktop-shell/ui` | Passed; 41 modules transformed |

> The desktop UI has no pre-existing automated component or end-to-end test
> harness. Per scope, no replacement harness was invented; the native page
> was validated by the production TypeScript and Vite build.

## Scope Controls

No `web-ui`, `admin-shell`, mobile client, Docker/e2e suite, or unrelated
Todo/Target/StaffLoan workflows were changed. The feature uses Tauri
commands and the shared local composition root rather than HTTP requests.

## Documentation Updated

The dated implementation and technical decision are recorded in
`IMPLEMENTATION_PLAN_User_Hierarchy.md` §11.10 and `DECISIONS.md` under
“Native desktop notifications — complete 2026-08-17.”
