# Rust/UI Integration Map

Source revision: `d8f64889cfe4473d40d50c41b0992bd22955eb7b`.

## APP-DS — desktop shell

The React UI communicates with the Tauri command layer using `@tauri-apps/api/core` `invoke`. The Rust command handler registered in `crates/bins/desktop-shell/src/lib.rs` includes the following commands:

| Command | UI caller / use | Rust-side behavior visible in source | Evidence |
|---|---|---|---|
| `get_current_session` | `ui/src/App.tsx` at startup | Loads stored session through `session::load`; returns identity/session metadata or `None`; token pair is deliberately omitted from `SessionInfo`. | `desktop-shell/src/lib.rs:302-353`; `ui/src/App.tsx` |
| `login` | `ui/src/pages/Login.tsx` | Calls `session::authenticate` against fixed ONYX Worker API base; saves session; rebuilds shared AppState scoped to returned organization; refreshes hierarchy cache best-effort; returns `SessionInfo`. | `desktop-shell/src/lib.rs:355-420`; `desktop-shell/src/session.rs:111-162` |
| `logout` | MainLayout, startup stale-server branch, Settings callback | Clears stored session, rebuilds AppState in a placeholder state, clears person-dependent hierarchy cache according to code comments. | `desktop-shell/src/lib.rs:422-452`; `ui/src/App.tsx`; `MainLayout.tsx` |
| `execute_command` | `useCommand` / mutation workflows (missions, tasks, messaging, approvals, file ACL actions) | Accepts command envelope, calls client composition command service, serializes outcome. | `desktop-shell/src/lib.rs:93-106`; `ui/src/hooks/useCommand` |
| `execute_query` | `useQuery` and data-reading page hooks | Accepts query envelope and serializes result. | `desktop-shell/src/lib.rs:108-133`; `ui/src/hooks/useQuery` |
| `subscribe_events` | Notification/event UI and integration hooks | Subscribes to event stream through Rust-side client composition/event bus. | `desktop-shell/src/lib.rs:135-164` |
| `get_sync_status` | MainLayout polls on initial mount and every 5 seconds | Reads sync status; layout retains previous status if a transient poll fails. | `desktop-shell/src/lib.rs:165-185`; `ui/src/components/Layout/MainLayout.tsx` |
| `upload_file` | Files upload form | Delegates to `file_upload_coordinator.upload_new_file`; serializes outcome. | `desktop-shell/src/lib.rs:187-225` |
| `download_file` | Files download controls | Delegates to file upload coordinator download flow; serializes written-byte count/outcome. | `desktop-shell/src/lib.rs:228-260` |
| `store_secret`, `get_secret`, `delete_secret` | Exposed commands for secret store use; a direct page caller was not established by the inspected UI paths | Delegate to `SecureStorage` implementation. | `desktop-shell/src/lib.rs:262-300`; `secure_storage/mod.rs` |

Other implementation facts:

- `SharedAppState` is an `Arc<RwLock<Arc<AppState>>>`, allowing successful login/logout to swap application state.
- `SessionInfo` exposed to the frontend contains username, is_admin, server address, user ID, organization ID and device ID; it omits access and refresh tokens.
- `StoredSession` persists access/refresh tokens and identity data through the `SecureStorage` port. That port defines store/get/delete operations and uses `KeyringSecureStorage` backed by `keyring::v1::Entry`, with synchronous entry calls placed in `spawn_blocking`.
- A source test checks that serialized `SessionInfo` omits access/refresh token fields. Source existence is not a test run result.
- The approval dialog submits typed command envelopes carrying expected version and current identity/device/org context. Modal keyboard behavior is implemented in React rather than as a native OS dialog.

## APP-AD — Admin shell

| Integration | UI side | Rust-side behavior | Evidence |
|---|---|---|---|
| Native sign-in | `ui/src/pages/Login.tsx` → `auth/nativeLogin.ts` → `invoke("admin_login")` | `src/login.rs::admin_login` accepts username/password/request ID, applies a 30-second HTTP timeout, posts to `/api/auth/login` with `client_type="admin"`, and returns safe error categories. | `admin-shell/ui/src/pages/Login.tsx`; `ui/src/auth/nativeLogin.ts`; `src/login.rs:15-90` |
| General Admin API client | `ui/src/api/client.ts` (Axios) | No generic Rust HTTP API proxy is exposed in `admin-shell/src/lib.rs`; the Rust command list is `admin_login`. | `admin-shell/src/lib.rs:19-23`; `ui/src/api/client.ts` |
| Dialog integration | UI uses `@tauri-apps/plugin-dialog` capability/plugin but native file dialog usage in the inspected Admin page source was not established. | Plugin initialized in `admin-shell/src/lib.rs`. | `admin-shell/src/lib.rs:19-22`; Admin Cargo manifest |
| Session storage | Zustand auth store and helpers persist auth session values in browser `sessionStorage`. | Rust login command does not return tokens to a dedicated vault in the observed Admin path; it returns the Worker JSON to the UI for validation/storage. | `admin-shell/ui/src/utils/auth.ts`; `ui/src/stores/authStore.ts`; `src/login.rs` |

## Error and state boundary

Desktop-shell exposes a serialized `ShellError` shape from Rust and the UI maps errors to user-facing messages. Admin-shell native login maps transport failures to categories (`http`, `timeout`, `connect`, `network`, `invalid_response`) and the UI maps these to login diagnostic codes/messages. Live timeout, offline, malformed-response, permission-denial, and OS keyring errors were not induced during this inventory.
