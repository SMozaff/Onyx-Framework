# Mobile Flutter Parity Matrix (M0 Frozen Baseline)

This document is the frozen (M0) parity baseline for the Flutter reference
implementation at `mobile/`, per ONYX-MOB-00 §8 / ONYX-MOB-01 §25. The
Flutter app is being frozen as a reference; a native Kotlin Android rewrite
begins in later tasks and MUST be checked against the behavior documented
here as its acceptance criteria. Nothing in this document is aspirational —
every line reflects what the code in `mobile/lib` does today, read in full,
not inferred from names. Where behavior is ambiguous or not fully
determinable from the code, that is stated explicitly rather than guessed.

Source files read in full for this matrix: `ui/screens/dashboard.dart`,
`ui/screens/missions.dart`, `ui/screens/mission_detail.dart`,
`ui/screens/tasks.dart`, `ui/screens/task_detail.dart`,
`ui/screens/approvals.dart`, `ui/screens/notifications.dart`,
`ui/screens/files.dart`, `ui/screens/settings.dart`,
`ui/ffi_login_screen.dart`, `ui/http_login_screen.dart`,
`ui/startup_error_screen.dart`, `ui/app.dart`, `net/auth.dart`, `main.dart`,
`lib/bridge/*.dart`, and `crates/mobile-core/mobile-core.h`.

---

## 1. Dashboard (`ui/screens/dashboard.dart`)

**What it does today:** A single scrollable "command center" screen, pull-
to-refresh via `RefreshIndicator` calling `controller.refresh()`. Shows a
2-column (or 4-column when screen width > 600px) stat grid of: Missions
count, Tasks count, Conflicts count, and Queued (i.e.
`controller.sync.pendingOutboxCount`). If `controller.error` is non-null, an
error card ("Local core unavailable") is shown with the raw
`error.toString()` and a manual refresh icon button. If
`controller.conflicts` is non-empty, a second warning card is shown
("Conflict review required", with the conflict count) — this is separate
from, and in addition to, the conflict banner already shown at the app-shell
level (`ui/app.dart`'s `_MobileShell`). Below that: an "Active missions"
section header with a "View all" button that calls
`controller.selectNavigation(1)` (switches bottom-nav tab to Missions, index
1) — no navigation via `Navigator`, this is a `IndexedStack` tab switch. If
`controller.missions` is empty, shows an empty-state card ("No missions
yet" / "Create the first mission from the Missions tab."); otherwise renders
up to the first 3 missions (`.take(3)`) as `MissionCard` widgets. Finally a
"Recent activity" card: if both missions and tasks are empty, shows a static
placeholder text ("Domain events will appear here..."); otherwise lists up
to 2 missions and up to 2 tasks (`.take(2)` each) as simple `ListTile`s
showing `"Mission {status} · version {version}"` / `"Task {status} ·
version {version}"`.

**Backend calls:** None directly — all data comes from already-loaded
`OnyxController` state (see §3). Pull-to-refresh calls `controller.refresh()`
which fans out the full shared refresh (mission/task/approval/notification
lists + sync status + conflicts).

**State read:** `controller.missions`, `controller.tasks`,
`controller.conflicts`, `controller.sync.pendingOutboxCount`,
`controller.error`. **State written:** none (read-only screen except for
`selectNavigation`, which only mutates `navigationIndex`).

**Non-obvious behavior:** The "Recent activity" feed is NOT a real domain
event log — it is a re-slice of the same mission/task lists already loaded,
independently limited to 2 items each (not chronologically sorted by any
visible logic; ordering is whatever `listAggregates` returned). Loading
state is handled at the app-shell level, not per-screen: while
`controller.isLoading` is true, the entire body is replaced by a spinner
(`_MobileShell`), so this screen itself has no independent loading state.

---

## 2. Missions list (`ui/screens/missions.dart`)

**What it does today:** Lists all `controller.missions` (no filtering, no
pagination) as `MissionCard` widgets, full list (not `.take(n)`). Pull-to-
refresh. A `FloatingActionButton.extended` ("Mission" + add icon) opens an
`AlertDialog` with Name (required, autofocus) and Description (optional)
text fields. On Create: only proceeds if `submitted == true` AND
`name.text.trim().isNotEmpty` — an empty name silently does nothing (dialog
already closed, no error surfaced). Calls
`context.read<OnyxController>().createMission(name, description)`, where an
empty/whitespace-only description is normalized to `null`.

**Backend calls:** `createMission` builds and executes a `CreateMission`
command envelope: `commandType: 'CreateMission'`, `targetType: 'mission'`,
`targetId: <freshly generated UUID>`, payload
`{'CreateMission': {name, description, owner_id: encodeId(userId)}}`, via
`api.executeCommand(api.buildCommandEnvelope(...))`. After success, calls
`controller.refresh()` (the full shared refresh, not just missions).

**State read:** `controller.missions`. **State written:** triggers a new
mission via `executeCommand`, then refreshes all shared state.

**Non-obvious behavior:** Empty-state card text: "No missions are stored on
this device." (distinct wording from Dashboard's empty-state). No
client-side validation beyond non-empty-name; no length limits enforced in
Dart. No loading/busy indicator on the Create dialog itself — the dialog
just closes immediately on submit and the app-shell's global loading state
does not re-trigger (since `isLoading` is only set to `true` initially and
by `refresh()`'s finally-block only sets it to `false`, so a subsequent
`refresh()` call from `createMission` does not show the full-screen
spinner — see §3, `isLoading` is never reset to `true` after the first
load).

---

## 3. Mission Detail (`ui/screens/mission_detail.dart`)

**What it does today:** Shown via navigation push (not part of the bottom
nav) with a `LoadedAggregate mission` passed in directly (a snapshot at
navigation time, not re-fetched). Displays title, optional description,
`StatusBadge` for status, and an "Authority state" card with object
version, lifecycle epoch, authority epoch, and raw ID. If
`mission.status == 'AwaitingApproval'` (`canDecide`), shows a "Review
approval request" card: a multi-line reason `TextField` and two buttons,
Reject and Activate.

- **Reject** (`RejectApproval`) is disabled unless the reason field is
  non-empty (trimmed) AND not busy.
- **Activate** (`ActivateMission`) is enabled whenever not busy — reason is
  optional for Activate.

On decide, calls `controller.decide(target: mission, targetType: 'mission',
commandType: <'RejectApproval'|'ActivateMission'>, reason: <trimmed text>)`.
On success, pops the screen (`Navigator.of(context).pop()`). On failure,
sets `_error` to `error.toString()` and displays it in red beneath the
buttons, screen stays open. A raw JSON "Raw local projection" of
`mission.aggregate` is shown in a collapsible `ExpansionTile` via
`SelectableText`, pretty-printed with 2-space indent.

**Backend calls:** `OnyxController.decide()` → `api.executeCommand` with a
command envelope: `commandType` = `'RejectApproval'` or `'ActivateMission'`,
`targetType: 'mission'`, `targetId: mission.id`, payload
`{commandType: {'reason': reason}}`, `expectedVersion: mission.version`,
`lifecycleEpoch: mission.lifecycleEpoch`, `authorityEpoch:
mission.authorityEpoch` (optimistic-concurrency fields taken from the
already-loaded aggregate, not re-fetched). On success, `decide()` calls
`controller.refresh()` (full shared refresh).

**State read:** the passed-in `LoadedAggregate mission` (a snapshot).
**State written:** on decide, one command execution plus a full controller
refresh.

**Non-obvious business rules:** Mission's approve/reject commands are named
`ActivateMission`/`RejectApproval` — NOT `ApproveMission`/`RejectMission`
and NOT the same names as Task's `ApproveTask`/`RejectTask`. The
Reject-requires-nonempty-reason / Approve-optional-reason rule matches Task
Detail exactly. The decide buttons are always shown when `canDecide` is
true regardless of whether the current actor is actually authorized —
authorization is enforced server/core-side (owner-authority gate); a denial
surfaces as the raw exception message from `CommandFailedException` (e.g.
"actor ... is not authorized to decide on behalf of owner ..."), not a
pre-emptive UI check, since there is no FFI query to check authority in
advance.

---

## 4. Tasks list (`ui/screens/tasks.dart`)

**What it does today:** Lists all `controller.tasks` as `TaskCard` widgets,
full list. Pull-to-refresh. FAB ("Task") opens a create dialog with a
`DropdownButtonFormField` of missions (defaulting to `controller.missions
.first`), a Title field (required) and Description field (optional). If
`controller.missions` is empty when the FAB is tapped, the dialog is never
shown — instead a `SnackBar` "Create a mission before adding tasks." is
shown immediately. On submit: proceeds only if `submitted == true` and
title is non-empty; calls `controller.createTask(mission: selectedMission,
title, description)`.

**Backend calls:** `createTask` builds/executes a `CreateTask` command:
`targetType: 'task'`, `targetId: <new UUID>`, payload `{'CreateTask':
{mission_id: encodeId(mission.id), title, description, owner_id:
encodeId(userId)}}`. Then `controller.refresh()`.

**State read:** `controller.tasks`, `controller.missions` (for the FAB
mission-existence gate and the dropdown). **State written:** as above.

**Non-obvious behavior:** Empty-state text: "No tasks are stored on this
device." The mission-required-before-task-creation rule is enforced purely
client-side (via the `SnackBar` short-circuit), not by disabling the FAB
itself.

---

## 5. Task Detail (`ui/screens/task_detail.dart`)

**What it does today:** Structurally identical to Mission Detail. Shows
title, description, `StatusBadge`, and an "Execution state" card (version,
lifecycle epoch, authority epoch, ID). If `task.status == 'Submitted'`
(`canDecide`), shows a "Review submission" card with a reason field and
Reject/Approve buttons.

- **Reject** (`RejectTask`) disabled unless reason is non-empty and not
  busy.
- **Approve** (`ApproveTask`) enabled whenever not busy; reason optional.

Same success/failure handling as Mission Detail (pop on success, inline red
error text on failure). Same raw-JSON `ExpansionTile` for `task.aggregate`.

**Backend calls:** `controller.decide(target: task, targetType: 'task',
commandType: <'RejectTask'|'ApproveTask'>, reason)` → same command-envelope
shape as Mission Detail, with `targetType: 'task'`.

**State read/written:** same pattern as Mission Detail — snapshot in,
`decide()` executes command + full `refresh()`.

**Non-obvious business rules:** Task's decision commands are
`ApproveTask`/`RejectTask` — a *different* pair of command names from
Mission's `ActivateMission`/`RejectApproval`, even though both are gated by
the same conceptual "owner authority" mechanism and share the identical
`{reason: String}` payload shape (this is why `OnyxController.decide()` is
one generic method parameterized by `commandType`/`targetType`, not four
near-duplicate methods). `canDecide` gates on task status `'Submitted'`
specifically (vs. Mission's `'AwaitingApproval'`) — the two aggregates use
different status vocabularies for "awaiting a decision."

---

## 6. Approvals (`ui/screens/approvals.dart`)

**What it does today:** This screen is explicitly a **filtered view over
already-loaded Task/Mission state**, not a separate aggregate or a distinct
backend query. It computes `pendingTasks = controller.tasks.where(status ==
'Submitted')` and `pendingMissions = controller.missions.where(status ==
'AwaitingApproval')`, and renders them as two lists of `ListTile`s (tasks
first, then missions), each navigating via `Navigator.push` to
`TaskDetailScreen`/`MissionDetailScreen` respectively on tap. Pull-to-
refresh calls `controller.refresh()`.

**Backend calls:** None directly — no `listAggregates('approval')` call is
made here (see below). Only `controller.refresh()` on pull-to-refresh
(shared with every other screen).

**State read:** `controller.tasks`, `controller.missions`.

**Non-obvious/important fact confirmed in code (source-commented, verified
against this file's own doc comment, which this matrix treats as
authoritative code documentation, not just narrative):** `controller
.approvals` (backed by `listAggregates('approval')`, still fetched every
`refresh()` — see §7) is loaded into `OnyxController` state but is **NOT
used by this screen at all**, and per the file's own doc comment no
aggregate is ever actually stored locally under the `'approval'` type — the
local client-composition repositories cover
mission/task/conversation/message/file_asset/upload_session/policy/
legal_hold/connection_request/notification, never "approval." A separate,
unrelated server-side generic `ApprovalAggregate` concept exists
(`api-server::routes::command`, `"approval.Approve"`/`"approval.Reject"`)
but has no owner-authority gate and is never wired into the local command
path, so it is irrelevant to this screen. Real approval gating is entirely
the Task/Mission decision commands documented in §3/§5.

---

## 7. Notifications (`ui/screens/notifications.dart`)

**What it does today:** Lists `controller.notifications` as read-only
`Card`/`ListTile`s (title, description-or-fallback-ID subtitle, trailing
`StatusBadge` for status). No tap action, no create/decide actions. Pull-
to-refresh.

**Backend calls:** none directly; relies on `controller.notifications`
loaded by the shared `refresh()` (`api.listAggregates('notification')`).

**Non-obvious behavior:** Empty-state text is explicit about the
limitation: "No local Notification aggregate is available yet. Remote
notification delivery remains available through the web client." — this
implies notifications ARE technically wired to a local aggregate type
(`listAggregates('notification')` is called), but in practice the app
currently has no code path that populates any local Notification
aggregates on this device, so the list is expected to be empty in the
current build. This is stated as-is; whether any local flow ever populates
it is not determinable from the files read.

---

## 8. Files (`ui/screens/files.dart`)

**What it does today:** Two independent forms: "Upload a file" (a single
text field for an on-device filesystem path, plus an Upload button) and
"Download a file" (content-hash text field + destination-path text field,
plus a Download button). No file-picker UI — the user must type a raw
filesystem path (explicitly documented in the file's own doc comment as a
deliberate simplification: no `file_picker` dependency exists in
`pubspec.yaml`, and this was flagged as a natural follow-up, not done
speculatively). On upload success, shows byte count and content hash
(`_lastUpload['size_bytes']`/`['content_hash']`) and keeps the last upload's
hash visible below the button. On download success, shows bytes-written
count. Both show the raw error text prefixed with "Upload failed:"/
"Download failed:" on failure. A single shared `_busy` flag disables both
buttons while either operation is in flight.

**Backend calls:** `controller.api.uploadFile(path)` and
`controller.api.downloadFile(hash, dest)` — backed by mobile-core's
`mobile_core_upload_file`/`mobile_core_download_file` FFI functions on the
local-first (FFI) transport. On the HTTP transport, these are explicitly
documented (in this file's own doc comment, cross-referencing
`net/onyx_http_api.dart`) as throwing an explicit "not implemented" error,
since `api-server` has no HTTP file route.

**State read/written:** purely local widget state (`_lastUpload`,
`_status`, `_busy`); does not touch `OnyxController`'s shared
missions/tasks/etc. state and does not call `controller.refresh()` after
either operation.

---

## 9. Settings (`ui/screens/settings.dart`)

**What it does today:** Three/four sections depending on active transport:

1. **Connection mode card** — a `RadioGroup` choosing `'ffi'` (Local-first)
   vs `'http'` (LAN), initialized from the saved `transport_mode`
   preference (defaulting to `'ffi'`) but tracked in local widget state
   until "Save mode" is pressed, which writes
   `preferences.setString('transport_mode', ...)` and shows a SnackBar
   telling the user to restart the app — **selecting a different radio
   value does not take effect until an app restart**, and the app does not
   restart itself here (unlike Sign Out, which does call `restartApp()`).
2. **Identity card** — branches on `controller.api is! OnyxHttpApi`:
   - FFI mode: shows read-only `organizationId`/`userId` text (no longer
     editable — this used to be free-text fields, explicitly documented as
     a fixed security hole: "anyone could type in an arbitrary
     organization/user UUID and have mobile-core act as that identity");
     an editable "Cloud relay endpoint" field with its own "Save" button
     calling `controller.saveRelayEndpoint(relay)` (also requires restart
     to take effect).
   - HTTP mode: shows read-only org/user text plus a note that credentials
     must be re-entered every restart (no relay field — not applicable to
     HTTP transport).
3. **Data source card** — a read-only summary: "Live server data" vs
   "Local-first database" (icon + subtitle), showing mission/task counts
   and, for FFI mode only, `sync.pendingOutboxCount`.
4. **Account card** — shown only for FFI mode. "Sign out" button clears
   `FfiSessionStorage`, removes `organization_id`/`user_id` preferences,
   sets `hasRealFfiSessionKey = false`, disposes the current `api`, and
   immediately calls `restartApp()` (no SnackBar/restart-prompt needed —
   this one takes effect immediately, unlike the mode/relay saves above).

**Backend calls:** none directly by this screen except through
`controller.saveRelayEndpoint` (local preference write only, no network/FFI
call) and the Sign Out flow's `controller.api.dispose()` +
`restartApp()` (which re-runs the full startup sequence).

**Non-obvious behavior:** The connection-mode radio and relay-endpoint save
both require a manual app restart to take effect (a SnackBar says so
explicitly) — the running `OnyxController`/`api` are not swapped live.
Sign Out is the one action here that is immediate. There is no way in this
screen to switch from HTTP mode back to FFI mode's real login without first
manually navigating away/back through a restart — this screen only ever
writes the preference; `main.dart::restartApp()` (next launch) is what
actually branches on it.

---

## 10. Login: `ffi_login_screen.dart` vs `http_login_screen.dart`

**When each is used** (determined from `main.dart::restartApp`, which is
the sole call site for both): on every app start/restart, the
`transport_mode` SharedPreferences value (default `'ffi'`, settable only
via the Settings screen, §9) decides:
- `transport_mode == 'http'` → always shows `HttpLoginScreen` (HTTP mode
  never persists a password, so it prompts fresh on every restart — this is
  a deliberate "no password persistence" design decision, not a bug).
- `transport_mode == 'ffi'` (default) → shows `FfiLoginScreen` **only if**
  the `hasRealFfiSessionKey` SharedPreferences flag is not yet set to
  `true` (fresh install, or after Sign Out/reset). If that flag is already
  `true` (a previously-successful FFI login), `restartApp()` skips the
  login screen entirely and opens mobile-core directly under the saved
  `organization_id`/`user_id`.

This is a fully deterministic transport-mode + session-flag branch — **not**
a build flavor/compile-time flag; it is decided per-launch from persisted
preferences.

**`FfiLoginScreen`:** Server address (defaults to
`http://192.168.1.1:3000`), Username, Password fields. On submit: (1) real
`POST /api/auth/login` via `OnyxHttpAuthApi.login` with
`client_type: 'mobile'`; (2) persists real `organization_id`/`user_id` (from
the login response's `user` object) plus `ffi_session.server_address`/
`ffi_session.username` to `SharedPreferences`, and sets
`hasRealFfiSessionKey = true`; (3) persists access/refresh tokens to
`FfiSessionStorage` (OS-backed secure storage) *before* the
`SharedPreferences` writes, specifically so a crash between the two writes
never leaves the session-flag set with no backing token; (4) opens
mobile-core via `initializeFfiMobileCore` under the real organization id;
(5) best-effort fetches and applies the org hierarchy
(`fetchHierarchyJson` → `api.setHierarchy`) — failure here is logged, not
surfaced as a login failure (fail-closed on approvals only, not a full
lockout); (6) subscribes to events; (7) pushes `OnyxControllerHost` via
`Navigator.pushReplacement` (not a second top-level `runApp`, since a
`MaterialApp` is already running).

**`HttpLoginScreen`:** Server (HTTP) address, WebSocket address,
Organization UUID, Username, Password fields — all typed manually including
the org UUID (no server-derived identity resolution step distinct from
`OnyxHttpApi.open`, which performs its own login internally). On success,
persists non-secret fields (`http_base_url`, `ws_base_url`,
`organization_id`, `http_username` — password never persisted) only after
success (so a failed attempt does not clobber a previously-good save);
subscribes to events; navigates via `Navigator.pushReplacement` to
`OnyxControllerHost` using `api.loggedInUserId` (server-derived from the
login response, since HTTP mode has no separate local `CommandEnvelopeFactory`
concept) and an empty `relayEndpoint` (Cloud Relay is meaningless for a
direct HTTP/LAN connection).

**Shared error mapping (`_friendlyFfiLoginError`/`_friendlyLoginError`,
intentionally kept in sync between the two screens):**
- `MobileAccessRestrictedException` → "Mobile access is not enabled for
  your user class in this organization. Ask an admin to enable it in
  Settings." — this is the ONLY login failure with a distinct, specific
  message.
- error text containing "connection"/"socketexception"/"timeout" → generic
  network-unreachable message.
- error text containing "401"/"invalid_credentials" → "Invalid username or
  password." — note this is a *generic* message; the real backend
  (`net/auth.dart`, confirmed by reading `auth.rs` directly) intentionally
  returns the exact same `INVALID_CREDENTIALS` error code for every
  credential failure mode (unknown user, wrong password, disabled account)
  — there is no more specific server-side distinction to surface here.
- anything else → raw `"Sign-in failed: $error"` fallback.

**client_type value confirmed:** the literal string `'mobile'` is sent as
`client_type` in the `POST /api/auth/login` request body, in exactly one
place in the whole `mobile/` tree: `mobile/lib/net/auth.dart:46`
(`OnyxHttpAuthApi.login`), used identically by both login screens since both
route through this same class.

---

## 11. Startup / error recovery (`ui/startup_error_screen.dart`, `main.dart`)

**Startup sequence (`main.dart::restartApp`):** wrapped in
`runZonedGuarded` at the very top of `main()` so any startup error —
synchronous or asynchronous, anywhere in the call chain — is caught rather
than leaving a blank screen. Branches on `transport_mode` then
`hasRealFfiSessionKey` as described in §10. For the FFI path with an
existing session: opens mobile-core (`initializeFfiMobileCore`), calls
`api.subscribeEvents()`, registers Android/iOS background sync
(`registerAndroidBackgroundSync`/`registerIosBackgroundSync`), fires an
un-awaited best-effort hierarchy refresh
(`refreshHierarchyBestEffort` — see below), then `runApp(OnyxControllerHost(...))`.
Any exception in this try block (mobile-core open failure, event-
subscription failure, etc.) is caught; if `api` was already constructed it
is disposed first (best-effort, swallowing a secondary dispose failure so
the *original* error is what's shown), then `runApp(StartupErrorApp(...))`.

**`refreshHierarchyBestEffort`** (also called periodically by
`OnyxController.initialize` every 45 minutes for FFI-mode sessions, chosen
to stay well inside the server's 1-hour access-token TTL): reads the saved
server address and access token; if either is missing, no-ops. Tries
`fetchHierarchyJson()` + `api.setHierarchy()` with the current access token
first; on failure, tries reading the stored refresh token and calling
`OnyxHttpAuthApi.refresh()`, persists the rotated tokens, and retries the
hierarchy fetch once. Both failure paths are logged via `debugPrint`, never
thrown — this never blocks or fails app startup/operation; approvals simply
stay fail-closed against a stale/empty hierarchy cache until the next
successful refresh or a fresh login.

**`StartupErrorApp` (shown on any caught startup failure):** a standalone
`MaterialApp` (deliberately not depending on `Provider`/`OnyxController`,
since that's exactly what failed to build). Shows: a friendly, best-effort
message derived from the raw error text via `_friendlyMessage` (matches
substrings `'mobile_core_new failed'`, `'Event subscription failed'`,
`'library'`/`'symbol'`; falls back to a generic "unexpected error"
message rather than guessing); a "Show/Hide technical details" toggle
revealing the raw error + stack trace in a monospace `SelectableText`; an
editable "Cloud relay endpoint" field with "Save and retry" (saves the
field if non-empty, then calls `restartApp()` again in place — no OS-level
relaunch); and "Sign out and retry" (`_resetToDefaults`) which clears
`FfiSessionStorage`, removes `organization_id`/`user_id` preferences, sets
`hasRealFfiSessionKey = false`, then retries — routing back to a real login
screen rather than reopening mobile-core under a stale/corrupt identity.
There is **no way to hand-edit `organization_id`/`user_id` directly** from
this screen — that was a fixed security hole (documented explicitly in the
file's own comment) allowing anyone to impersonate any org/user with zero
authentication; the only recovery path for a bad identity is full sign-out.

---

## 12. Shared refresh / controller architecture (`ui/app.dart`)

`OnyxController` (a `ChangeNotifier`, provided app-wide via
`ChangeNotifierProvider`) is the single source of truth for all cross-screen
state. Its `refresh()` method is the **only** place that fans out real
backend/FFI calls for list data, and it fans out exactly six calls in
parallel via `Future.wait`:

1. `api.listAggregates('mission')` → `controller.missions`
2. `api.listAggregates('task')` → `controller.tasks`
3. `api.listAggregates('approval')` → `controller.approvals` (loaded, but —
   per §6 — not actually populated with real data and not read by any
   screen read during this review, including `ApprovalsScreen` itself)
4. `api.listAggregates('notification')` → `controller.notifications`
5. `api.getSyncStatus()` → `controller.sync`
6. `api.listConflicts()` → `controller.conflicts`

If any of the six calls throws, `controller.error` is set to the caught
object and none of the six lists/fields are updated for that refresh cycle
(the `try` block assigns all six only after `Future.wait` resolves
successfully; a partial-success fan-out is not possible — it's all-or-
nothing per refresh). `isLoading` is set to `false` in the `finally` block
regardless of outcome (note: it is initialized to `true` and never reset to
`true` again after the very first load, so subsequent `refresh()` calls —
whether from pull-to-refresh, `createMission`, `createTask`, `decide`, or
`resolveConflict` — do not re-trigger the app-shell's full-screen loading
spinner; the shell keeps showing existing data while `refresh()` runs
silently in the background). `notifyListeners()` is called once, in the
`finally` block, after every `refresh()` attempt.

**The key architectural property for parity:** every screen documented in
§1–§10 (Dashboard, Missions, Tasks, Notifications, Approvals) reads
`controller.missions`/`controller.tasks`/`controller.notifications`/
`controller.approvals`/`controller.conflicts`/`controller.sync` directly
from this already-loaded shared state via `context.watch<OnyxController>()`.
**No screen independently re-queries the backend for its own list data** —
the only calls screens make themselves are mutation calls (`createMission`,
`createTask`, `decide`, `resolveConflict`, `triggerSync`,
`saveRelayEndpoint`, file upload/download), each of which (except file
upload/download and `saveRelayEndpoint`) itself ends by calling the same
shared `refresh()` to re-sync all six lists at once, not just the list
relevant to that mutation. `Mission Detail`/`Task Detail` are a partial
exception: they take a `LoadedAggregate` snapshot as a navigation argument
rather than watching the controller list live, so their displayed
version/status is frozen as of navigation time until a decide/refresh event
causes the underlying screen to rebuild and re-navigate.

**Other `OnyxController` responsibilities:** subscribes to `api.events` and
calls `refresh()` on every event (so any server-pushed event triggers a
full six-call refresh, not an incremental update); tracks network
connectivity via `connectivity_plus` (`hasNetwork`), surfaced as an offline
`MaterialBanner` in `_MobileShell`; runs the 45-minute hierarchy-refresh
timer for FFI-mode only (`api is OnyxMobile` check — HTTP mode has no local
hierarchy cache or token to renew); disposes the event subscription,
connectivity subscription, hierarchy timer, and the underlying `api` on
`dispose()`.

---

## 13. FFI contract surface (`crates/mobile-core/mobile-core.h`)

Grepped directly from the header (function-signature lines only). The
functions actually invoked from Dart, confirmed via grep of
`mobile/lib/bridge/*.dart` for `mobile_core_`, are:

- `mobile_core_new`
- `mobile_core_free`
- `mobile_core_free_string`
- `mobile_core_execute_command`
- `mobile_core_execute_query`
- `mobile_core_list_aggregates`
- `mobile_core_get_sync_status`
- `mobile_core_list_conflicts`
- `mobile_core_resolve_conflict`
- `mobile_core_trigger_sync`
- `mobile_core_set_hierarchy`
- `mobile_core_subscribe_events`
- `mobile_core_unsubscribe`
- `mobile_core_upload_file`
- `mobile_core_download_file`

— 15 distinct functions called from Dart.

The full header additionally declares three functions **not** called
anywhere under `mobile/lib/bridge/`:

- `mobile_core_background_sync_registered`
- `mobile_core_android_do_work` (the Android-specific background-work entry
  point)
- `mobile_core_ios_background_sync` (the iOS-specific background-sync entry
  point)

giving **18 total functions** declared in `mobile-core.h`.

**Discrepancy vs. this task's framing:** the task prompt describes "the 17
mobile-core functions plus the Android-specific one" (implying 18 total,
with exactly one platform-specific function). The header as read directly
contains **two** platform-specific functions
(`mobile_core_android_do_work` for Android and `mobile_core_ios_background_sync`
for iOS), not one, alongside `mobile_core_background_sync_registered` (a
platform-neutral status query used by neither's background-service Dart
file as confirmed by the bridge grep above — it is not called from
`mobile/lib/bridge/*.dart` at all in the files this review reached, though
it may be called from `background/android/workmanager_service.dart` or
`background/ios/background_service.dart`, which were not part of the file
list read for this review and were only referenced, not opened). The 15
Dart-called functions plus these 3 header-only functions total 18, matching
the prompt's implied total count (17 + 1 = 18) but not its implied
breakdown (this review found 15 "core" functions called from
`lib/bridge/`, not 17, and 3 platform/status-related functions declared but
not called from `lib/bridge/`, not 1).

**Resolved:** none of the three are called directly from Dart at all —
they're invoked from native platform code that Dart's background scaffolding
delegates to. `mobile_core_android_do_work` is called from Kotlin, not Dart:
`android/app/src/main/kotlin/com/onyx/WorkManagerService.kt`'s
`nativeAndroidDoWork()` is an `external fun` bound via
`System.loadLibrary("mobile_core")`, invoked from `doWork()`.
`mobile_core_background_sync_registered` is called from Swift, not Dart:
`ios/Runner/BackgroundService.swift` resolves it via `dlsym` at runtime.
`mobile_core_ios_background_sync` was not found called from any file
reached in this review (Dart, Kotlin, or the one Swift file read) — a
remaining, real gap, not a resolved one; it may be dead code, called from
another Swift file not read here, or genuinely unwired. So the real
breakdown is 15 functions called from Dart's `lib/bridge/`, 2 called from
native platform code (`android_do_work` from Kotlin,
`background_sync_registered` from Swift), and 1
(`mobile_core_ios_background_sync`) not confirmed called from anywhere
reached by this review.

---

## 14. Real Automated Test Baseline (M0, re-run fresh)

Run against this task's real tip (`claude/onyx-pending-fixes-6l2hhk`,
carrying H10), via `flutter test` and `flutter analyze` in `mobile/`:

```
flutter analyze  ->  No issues found! (ran in 16.7s)
flutter test     ->  16 passed, 1 skipped, 0 failed  ("All tests passed!")
```

- `test/bridge_test.dart` — 3 tests, all passed (UUID conversion, command
  envelope authority/causality fields, bridge abstraction executes
  commands/queries).
- `test/integration/navigation_test.dart` — 1 test, passed (all primary
  mobile screens reachable).
- `test/integration/p2p_sync_test.dart` — 1 test, **skipped**, real and
  disclosed: `Skip: Requires two authorized iOS/Android devices and
  ONYX_MOBILE_DEVICE_TEST=1`. Not a failure — this test genuinely cannot
  run without two real physical/authorized devices, and is gated behind
  an explicit opt-in environment variable rather than silently
  no-op'd.
- `test/unit/dashboard_test.dart` — 1 test, passed.
- `test/unit/approvals_test.dart` — 4 tests, all passed (filters to
  Submitted tasks / AwaitingApproval missions only; tapping a pending task
  opens `TaskDetailScreen` with real Approve/Reject actions; a
  non-awaiting task shows no decision actions; Reject stays disabled
  until a reason is entered, Approve does not require one).
- `test/unit/conflict_dialog_test.dart` — 1 test, passed (compares values,
  accepts a resolution).
- `test/unit/sync_status_test.dart` — 1 test, passed (offline/online/
  queued/conflict badge states).
- `test/integration/background_sync_test.dart` — 1 test, passed
  (background sync delegates to the Rust bridge abstraction).

**Parity floor for the Kotlin rewrite:** 16 passing, 1 legitimately
skipped (real-device-gated, not a stub to silently drop), 0 failing. Any
Kotlin test suite claiming parity must not regress below this — in
particular, the four `approvals_test.dart` behaviors (filtered view
membership, reason-required Reject gating, Approve requiring no reason)
are precise, checkable acceptance criteria, not just "the screen exists."
