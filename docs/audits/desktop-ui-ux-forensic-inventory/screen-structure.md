# Screen-by-screen structural description

Static source register for commit `d8f64889cfe4473d40d50c41b0992bd22955eb7b`. “Loading/error/empty” states below refer only to states visible in the inspected source or hook contract; no live API execution was performed.

## APP-DS — ONYX desktop shell

### DS-01 Login
Source: `crates/bins/desktop-shell/ui/src/pages/Login.tsx`. A two-region responsive authentication shell: a branded hero/operation context area and a credential card. The card contains a username input, password input, locale selector, submit button, loading label and conditional alert text. Submission invokes Rust Tauri command `login` with username/password, then navigates to `/` and passes the returned identity to the app; errors are mapped to localized messages. Browser autocomplete values are `username` and `current-password`; both inputs have `required`. Token persistence is native Rust-side, not a page field.

### DS-02 Session loading
Source: `crates/bins/desktop-shell/ui/src/App.tsx`. On mount, invokes `get_current_session`; during the unresolved call, displays centered `common.loadingSecureSession` text. A retry token re-runs the load. It does not expose the operational routes during loading.

### DS-03 Startup error
Source: `crates/bins/desktop-shell/ui/src/App.tsx`. A `role="alert"` section contains an error title, message and Retry button. The button increments the retry token, restarting the session-load effect. The actual network/IPC error categories depend on Rust and `toUserFacingError`.

### DS-04 Dashboard
Source: `crates/bins/desktop-shell/ui/src/pages/Dashboard.tsx`. Uses summary/stat cards and links to major staff operations. Values are data-derived; the full rendered number and arrangement of data-dependent cards cannot be counted from the source without the live state.

### DS-05 Missions
Source: `crates/bins/desktop-shell/ui/src/pages/Missions.tsx`. Handles the `/missions` and `/missions/:missionId` route patterns with one component. Source includes a mission ID lookup input and View action, a refresh action, and a create-mission workflow with name and description. Operations go through the command/query hooks and typed command envelope.

### DS-06 Tasks
Source: `crates/bins/desktop-shell/ui/src/pages/Tasks.tsx`. Handles `/tasks` and `/tasks/:taskId` using one component. Includes task lookup, refresh, a task create workflow (title, description, optional mission ID), and a Mark Ready action. Detail/selection content depends on query state and provided route parameter.

### DS-07 Approvals
Source: `crates/bins/desktop-shell/ui/src/pages/Approvals.tsx`, with `components/ApprovalDialog/index.tsx` and `components/Dialog/AccessibleDialog.tsx`. Contains task ID lookup and an action that opens an in-webview modal dialog. Dialog displays a task title, a decision-note textarea and Cancel, Reject, Approve controls. Reject is disabled when the reason is blank; Approve is enabled while not loading even if the note is empty. The handler emits `ApproveTask` or `RejectTask` command with task ID, organization/user/device IDs and expected task version. Dialog traps Tab focus, focuses the configured textarea at open, restores prior focus at close, and closes on Escape unless busy.

### DS-08 Messaging
Source: `crates/bins/desktop-shell/ui/src/pages/Messaging.tsx`. One component serves `/messaging` and `/messaging/:conversationId`. The file defines lookup and compose/edit/action subcomponents: conversation type selection, optional parent-supergroup ID, member addition by ID and a checkbox for parent-supergroup membership, message lookup, post/edit message fields, reaction code and add/remove actions, redaction, and connection request send/accept/decline/revoke workflows. Many controls are conditional on selected conversation/message and command loading state; rendered counts are not stable from source alone.

### DS-09 Files
Source: `crates/bins/desktop-shell/ui/src/pages/Files.tsx`. One component serves `/files` and `/files/:fileAssetId`. Includes asset-ID lookup, upload destination/path field, version listing and download destination, per-version download buttons and user access actions (grant/revoke, quarantine, archive). The visible UI takes a path-like string for upload/download; the inspected form does not use a browser file picker for this upload field. Rust commands `upload_file` and `download_file` are registered by the desktop shell.

### DS-10 Notifications
Source: `crates/bins/desktop-shell/ui/src/pages/Notifications.tsx`. Notification collection view with Refresh and per-notification acknowledge controls. Feed content and number of actions depend on returned data; the presence of a notification event validator is source-confirmed, but live event behavior is not runtime-confirmed.

### DS-11 Account settings
Source: `crates/bins/desktop-shell/ui/src/pages/Settings.tsx`. A single account section shows the signed-in username and a translated cloud-service note. A Log Out action calls the supplied reauthentication/logout handler. It is not a general preference page in the current component source.

## APP-AD — ONYX Admin

### AD-01 Login
Source: `crates/bins/admin-shell/ui/src/pages/Login.tsx`. Hero and credential-card layout with username/password fields, locale selector and submit button. UI uses native Tauri `admin_login` rather than making the first credential request from the WebView. Submission records a request ID and diagnostic outcome, maps errors to an alert and routes to `/` after a valid session is stored. Password and tokens are not included in the diagnostic fields documented by the handlers.

### AD-02 Lazy route loading
Source: `crates/bins/admin-shell/ui/src/App.tsx`. Suspense fallback shows centered `common.loadingAdmin` text while one of the lazy page modules resolves.

### AD-03 Users
Source: `crates/bins/admin-shell/ui/src/pages/Users.tsx`. The directory table exposes username/admin-state fields and each user's class, parent and active status. A creation workflow collects username, email, password and class. Existing row actions set class, assign parent, and deactivate/reactivate. The user collection length determines row/control counts. API calls in the source include `GET /api/admin/users`, `POST /api/admin/users`, and user-specific class/parent/status paths.

### AD-04 Policy and settings
Source: `crates/bins/admin-shell/ui/src/pages/Settings.tsx`. A single settings component hosts separate panels for mobile-access allowed classes, ID lookup, policy creation, policy version/rule drafting and actions, and legal holds. Controls are conditional on selected policy/hold ID and request state. The source includes class checkboxes, policy-name/key/value inputs, rule-type select, rule add/submit, publish/retire/evaluate, and legal-hold target UUID/type/reason and release lookup. Exact live row counts depend on fetched records and dynamic rules.

### AD-05 Staff profiles
Source: `crates/bins/admin-shell/ui/src/pages/Profiles.tsx`. Lists profile records, allows opening an edit overlay, and exposes batch import/export. The profile shape in TypeScript includes profile/owner IDs, full name, photo blob key, job title, contact email/phone, department, class label, parent display name and a string-array team memberships field. Editable scalar fields are rendered through a reusable Field component; team memberships use a separate control. Import accepts only CSV/JSON extensions and MIME values in inspected source. Export buttons produce CSV or JSON files.

### AD-06 Organizations
Source: `crates/bins/admin-shell/ui/src/pages/Organizations.tsx`. Lists organization cards, creates an organization from a required name (maximum 160 characters), allows renaming and activation/deactivation for eligible organizations, and displays selected organization's Admin accounts. The admin creation workflow collects username, contact email and initial password (minimum length 8); these fields/actions are disabled when busy or the selected organization is inactive. The organization route and navigation item are client-gated by role `ALL_FATHER`; the source explicitly says backend authorization is the actual boundary.

### AD-07 Admin access required
Source: `crates/bins/admin-shell/ui/src/App.tsx`. An authenticated user whose stored `is_admin` flag is false receives an access-required message naming the username and a Log Out control rather than the admin workspace.

## Overlays and nested state

- **DS-DLG-01 Approval dialog:** `desktop-shell/ui/src/components/ApprovalDialog/index.tsx`; rendered only when its `open` property is true.
- **AD-DLG-01 Profile edit overlay:** `admin-shell/ui/src/pages/Profiles.tsx`; opened by the New/Edit actions and closed/saved via callback. It is a React-rendered overlay, not a separately registered native window.
- Organization selection and admin panels, policy version/rule subpanels, message details, connection actions, and file version/ACL controls are conditional regions inside their owning page rather than declared top-level windows.

The report separates route templates from overlays. A complete DOM snapshot count for each state has not been obtained.
