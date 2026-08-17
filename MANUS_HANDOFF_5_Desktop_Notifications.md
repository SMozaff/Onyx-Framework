# Task for Manus: add Notifications to the desktop app (desktop-shell)

## Context

Read `DECISIONS.md` and `IMPLEMENTATION_PLAN_User_Hierarchy.md`
(newest entries first) before starting, plus this prompt in full — the
scope here is more specific than it might look at first, and getting
the architecture right matters more than the UI polish.

**This is about `crates/bins/desktop-shell`, the native Tauri desktop
app — not `web-ui` (the browser app) and not `admin-shell`.**
`web-ui` already has a working Notifications page
(`web-ui/src/pages/Notifications/index.tsx`) talking to `api-server`
over real HTTP — that one is fine, don't touch it, and don't just copy
its code, because `desktop-shell` uses a completely different data
layer (see below).

## The real gap, precisely

`desktop-shell` has **zero notification code** — confirmed by search,
not assumed. This isn't a partially-built feature, it's genuinely
absent: no page, no store, no route, nothing.

**Why it's not a simple port from `web-ui`**: `web-ui` talks to
`api-server` over HTTP (`/api/command`, `/api/query`). `desktop-shell`
is architecturally different — it's a Tauri app whose React frontend
calls generic Rust-side Tauri commands (`execute_command`,
`execute_query` in `crates/bins/desktop-shell/src/lib.rs`), which
dispatch by type string through `client-composition`'s
`command_registry`/`query_registry` (see `Approvals.tsx`'s
`useQuery("GetTask", targetId)` call for the working pattern to
follow).

**The actual blocker**: `client-composition` has **no `notification`
wiring at all** — no `"notification"` aggregate type registered in its
command/query registries (confirmed by search:
`crates/applications/client-composition/src/app_state.rs` has zero
references to notifications, unlike `"RequestApproval"`/
`"RejectApproval"`, which are registered there). Before any UI can be
built, `client-composition` needs `notification`'s command/query
handling wired in, the same way `policy`/`legal_hold` were wired into
`api-server` earlier this session (see `DECISIONS.md`'s entries on
that for the pattern, even though that was a different crate — the
principle of "register the aggregate type, wire load/query dispatch"
is the same).

## What to build, in order

### 1. Wire `notification` into `client-composition`

Register the `notification` aggregate type in
`client-composition`'s command/query registries, following whatever
pattern the existing aggregate types there use (check how
`approval`/`mission`/`task` are registered — `NOTIFICATION_DECISION_COMMANDS`-style
constant, repo wiring, etc., mirroring the naming/shape of the
existing ones). At minimum this needs:
- A way to list/query notifications (for the page's initial load).
- The `Acknowledge` command (marking one read — this already exists
  as a real command elsewhere in the codebase; check
  `api_server::routes::command`'s `"notification.Acknowledge"` arm and
  `NotificationAggregate`/`NotificationCommand` in
  `crates/bins/api-server/src/routes/command.rs` for the exact shape
  to replicate — `client-composition` needs the equivalent, not a
  reinvention).

### 2. The desktop UI itself

New page in `crates/bins/desktop-shell/ui/src/pages/Notifications.tsx`,
registered in `App.tsx`'s route table and `MainLayout`'s nav (follow
the exact pattern of the existing pages there — `Approvals.tsx` is
the closest structural match: list + per-item action).

**Real-time delivery is likely already available for free** — verify
this rather than assuming: `desktop-shell`'s `subscribe_events` Tauri
command already forwards every event on the org's event bus to the
webview as an `"onyx:event"` browser event
(`crates/bins/desktop-shell/src/lib.rs`). If notification-domain
events (e.g. a new notification being created) flow through that same
event bus once wired in step 1, the frontend just needs to listen for
`onyx:event` and refetch/update rather than needing new push
infrastructure. Confirm this is true before building a polling
fallback — don't build both if the event bus already covers it.

### 3. Tests

- Rust side (if you add or modify anything in `client-composition`):
  `cargo check`/`cargo clippy -- -D warnings`/`cargo test` for that
  package, plus a real integration test exercising the new
  notification command/query wiring (not a mock) — follow the
  existing test conventions in that crate.
- Frontend side: `desktop-shell/ui` currently has no `node_modules`
  installed — `npm install` first. Then `npx tsc -b` and
  `npx vite build` should stay clean (currently: 39 modules, builds
  successfully with zero notification code — confirm it still builds
  clean with the new page added). Add tests for the new page following
  whatever test setup already exists in `desktop-shell/ui`, if any —
  if there's no existing test harness there, note that rather than
  inventing a new one from scratch as a large side-project.

## What NOT to do

- Don't touch `web-ui`'s Notifications page — it already works, this
  is a separate app.
- Don't reuse `api-server` HTTP calls from `desktop-shell` — that
  would be architecturally inconsistent with every other page in this
  app, which all go through the Tauri command/`client-composition`
  path. If you find yourself wanting to add `fetch()`/`axios` calls to
  `desktop-shell/ui`, stop — that's the wrong layer.
- Don't build new push/polling infrastructure without first confirming
  the existing `onyx:event` mechanism doesn't already cover it (see
  step 2).
- Don't touch mobile (`mobile/`), the Docker/e2e suite, or anything
  already marked resolved in `DECISIONS.md`.

## Deliverable

Same standard as before: a short report on what was built and how it
was verified, plus a dated entry in `DECISIONS.md` and
`IMPLEMENTATION_PLAN_User_Hierarchy.md` in those same files, not a
separate changelog.
