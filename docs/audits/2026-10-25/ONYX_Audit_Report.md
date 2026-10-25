# ONYX Framework — Product Knowledge, Operational, and Desktop UI Audit

**Audit date:** 2026-10-25  
**Repository:** [SMozaff/Onyx-Framework](https://github.com/SMozaff/Onyx-Framework)  
**Branch / revision:** `main` @ `d8f64889cfe4473d40d50c41b0992bd22955eb7b`  
**Audit mode:** Read-only source and repository-metadata inspection; no application/source/configuration changes, migrations, credential actions, deployments, or production writes.  
**Overall completion status:** **PARTIAL — broad evidence-based audit completed; full forensic/runtime acceptance criteria remain open.**

> This is a consolidated master report for the two prompts in `ONYX_Audit_Prompts.md`. Findings distinguish current source, documentation, CI evidence, and runtime facts. The report does not treat code presence as proof of a working live workflow. Supporting structured registers are included in the accompanying audit pack.

## 1. Executive findings

ONYX is currently described by the repository as a “Mission Operations Platform” and a connected workspace for organizational work. Its current source combines mission/task operations, approvals, messaging, file assets and access controls, notifications, staff/profile administration, policy settings, mobile-access policy, organization and administrator management, audit events, push subscriptions, and relay/synchronization infrastructure. It is not evidence-backed to reduce the product to a general CRUD management app, nor to assume every planned business module exists.

The repository has two separate Tauri desktop applications: **ONYX** (`desktop-shell`) and **ONYX Admin** (`admin-shell`). They each configure one 1280×800 main window. The operational desktop UI is React/TypeScript with React Router, Tauri 2 IPC, localization, Tailwind CSS, and Rust-side command/query dispatch. The Admin UI is a separate React/TypeScript Tauri shell with role-aware routes and direct HTTP calls to the Cloudflare Worker.

The backend boundary is important: the documentation expressly describes two independent backends, not a single chain. The Cloudflare Worker at `deploy/cloudflare/edge-gateway` is documented as the supported remote API for Admin, Staff, web, and mobile clients and uses D1 for identity/organization/session authority and Durable Objects for event/relay hubs. The native Rust Axum API and Rust background worker are separate runtimes with SQLx-backed PostgreSQL/SQLite adapters. The Worker does not proxy to Axum, and a shared data store, replication guarantee, transaction, or cross-runtime authorization guarantee must not be inferred without concrete implementation evidence. [Backend architecture](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/BACKEND_ARCHITECTURE.md#L3-L47)

The current All-Father authentication path is ONYX username/password through the Cloudflare Worker, not Google/Supabase exchange. The authoritative canonical identity is `id='__allfather__'`, username `allfather`, organization `__allfather__`, active status, explicit `ALL_FATHER` role, and `is_admin=1`. Worker routes re-read current account/organization/role/revocation data from D1. GitHub Actions secrets are only inputs to the isolated local smoke workflow; they do **not** provision the live D1 account. The authentication model says production login remains unverified until owner-approved provisioning and a secret-safe live smoke test succeed. [Authentication model](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/SECURITY_AUTHENTICATION_MODEL.md#L1-L45)

The latest `main` CI run observed during this audit, run **40198813416**, completed successfully against the audited main revision. That is meaningful CI evidence, not proof that all user journeys work in a built Windows/Linux application or that the live production All-Father account is provisioned. A repository completion report also records a successful isolated All-Father smoke run (run 40060958495) and eight failed manual provisioning workflow runs; the same report explicitly says production provisioning and cache deletion were not performed. These details are separate from the current main CI result. [CI completion report](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/.github/COMPLETION_REPORT.md#L65-L80) [Provisioning limitations](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/.github/COMPLETION_REPORT.md#L115-L161)

The product history contains an explicit Flutter retirement decision. Current `docs/Structure.md` confirms the old `mobile/` Flutter tree is absent and identifies current mobile surfaces as native Kotlin Android (`mobile-android/`), a mobile observer PWA (`mobile-pwa/`), and Rust mobile core/JNI crates. Historical Flutter/iOS plans and Rust source gated for Apple targets must not be confused with a complete, currently supported iOS app. No reliable implementation evidence for the recalled exact phrases “Tech to Select”, “Modular Managing Platform”, or the “spiral” product concept was located in the searched current default-branch sources. Those memories remain unresolved historical questions, not requirements proven by this checkout.

The organizational model supports organization-scoped users, role/class metadata, `parent_user_id` reporting relationships, manager flags, and profile metadata. The Worker API contract includes a user hierarchy endpoint and the Admin UI supports organization-scoped user administration and profile CSV/JSON transfer. It does **not** establish a first-class `departments` or `teams` database entity, a dedicated interactive organization-chart/spiral module, or Excel/XLSX import. The profile transfer implementation accepts CSV and JSON. Current Worker account roles are `ALL_FATHER`, `ORGANIZATION_ADMIN`, and `STAFF`; management titles are modeled separately as operational class values such as `top_level_manager`, `senior_manager`, `team_leader`, `supervisor`, and `staff`.

No standalone timesheet or payment module/route was found in the inspected current repository areas. Messaging, file assets, and relay transport exist as code paths, but a working standalone optional-module system with install/activation/deactivation for each business module was not established. File transfer and relay primitives should not be conflated with an independently packageable “file relay module.”

## 2. Audit basis, method, and limitations

### 2.1 Evidence labels

- **Source-confirmed:** visible in current source or configuration at the pinned revision.
- **Test/CI-confirmed:** supported by a recorded CI result or test referenced in repository evidence. This does not independently reproduce the test in this environment.
- **Documentation-only:** stated in documentation/history but not independently established as runtime behavior.
- **Unverified:** no runtime click-through, device test, deployment probe, or local build was performed for that claim.
- **Not found in inspected sources:** bounded absence statement; not proof that a feature could never exist elsewhere.

Confidence is **high** when a current source definition directly establishes the claim, **medium** when it depends on source plus historical docs or an indirect integration trace, and **low** when history/source coverage is incomplete. Numerical probability estimates are not assigned to live workflow success because there is no sampled runtime evidence from which to calculate them.

### 2.2 Repository revision and coverage

The GitHub tree for `main` at `d8f64889cfe4473d40d50c41b0992bd22955eb7b` was available and reported non-truncated: **1,365 tree entries** (998 file blobs and 367 directory entries). The workspace declares **43 Cargo members**. A targeted source audit covered root product/legal/workspace files; current architecture and auth documents; the Worker route contract, auth/authorization source and selected D1 migrations; both Tauri configurations and route tables; all nine operational desktop page modules; Admin route/layout and five Admin page modules; desktop Rust login/session/command integration; selected Android session/screen source; PWA package metadata; CI completion notes; and selected PR records (#267, #268, #295, #303, #304, #325).

The inspection used the GitHub connector rather than a local repository checkout because a local clone could not be obtained in this environment. No local `cargo test`, `cargo build`, `npm test`, `npm run build`, Tauri packaging, Android install/device test, Windows/Linux click-through, production auth request, live D1 read, or production provisioning was run by this audit. A browser-level attempt to open the Worker `/health` and `/ready` URLs was not accessible through the browsing tool, so current live service availability is unverified.

This is therefore an evidence-backed **source audit**, not a fully runtime-complete forensic inventory. In particular, the requested exhaustive enumeration of every rendered control instance, every field validation, all keyboard/focus behavior, all animations and assets, and all platform behavior remains incomplete where the report records static source occurrences or unresolved evidence. Companion files name these boundaries instead of representing them as completed tests.

## 3. Repository and application inventory

| ID | Application/runtime | Current source location | Technology and role | Audit status |
|---|---|---|---|---|
| APP-DESK | ONYX operational desktop | `crates/bins/desktop-shell/` and `ui/` | Tauri 2, Rust, React/TypeScript; operations, local command/query access, session UI, missions/tasks/approvals/messaging/files/notifications/settings | Source inspected; Windows/Linux runtime unverified |
| APP-ADMIN | ONYX Admin desktop | `crates/bins/admin-shell/` and `ui/` | Tauri 2, Rust login command, React/TypeScript; accounts, policy, profiles, organization administration | Source inspected; Windows/Linux runtime unverified |
| APP-WEB | ONYX web UI | `web-ui/` | React/TypeScript SPA, Axios/Zustand/React Query/Recharts in manifest | Package/architecture evidence inspected, not a full UI screen audit |
| APP-ANDROID | Native Android | `mobile-android/` | Kotlin + Android UI, Rust JNI/mobile core, ONYX authentication/session | Selected source inspected; physical device/E2E unverified |
| APP-PWA | Mobile Observer PWA | `mobile-pwa/` | React/Vite; package description calls it read-only observer client; separate tests/type-check/browser scripts declared | Manifest inspected, browser/runtime unverified |
| APP-WORKER | Supported remote API | `deploy/cloudflare/edge-gateway/` | Cloudflare Worker + D1 + Durable Objects | Source/docs inspected; deployed behavior unverified |
| APP-AXUM | Native Rust API/worker | `crates/bins/api-server/`, `crates/bins/worker/` | Axum API and Rust worker with Rust domain/application layers and SQLx adapters | Architecture/source inspected; not same backend as Worker |

The root repository declares proprietary licensing in `LICENSE.md`; Cargo metadata points to that license file and sets `publish = false`. GitHub API metadata reports `NOASSERTION` rather than a standard SPDX license identifier. The source does not support describing this repository as MIT-licensed or generally permissively licensed. [License notice](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/LICENSE.md) [Workspace manifest](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/Cargo.toml)

## 4. Windows/Linux desktop windows and routes

### 4.1 Configured windows

| Window ID | Application | Product name / title | Identifier | Configured dimensions | Min dimensions/resizable | Evidence |
|---|---|---|---|---|---|---|
| WIN-DESK-01 | ONYX | `ONYX` | `com.onyx.platform` | 1280×800 | No minimum width/height or explicit resizable flag found in inspected config; behavior not runtime-tested | [`tauri.conf.json`](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/desktop-shell/tauri.conf.json#L1-L48) |
| WIN-ADMIN-01 | ONYX Admin | `ONYX Admin` | `com.onyx.admin` | 1280×800 | No minimum width/height or explicit resizable flag found in inspected config; behavior not runtime-tested | [`tauri.conf.json`](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/admin-shell/tauri.conf.json#L1-L51) |

Each inspected Tauri configuration defines one top-level window. The source also contains in-window React dialogs/overlays (including a review-submission dialog and a profile editor overlay); they are not separately registered Tauri top-level windows in the inspected route/configuration definitions. `bundle.targets = "all"` and the OS resource mappings configure multiple packaging targets; this does not establish that packaged artifacts were executed on every platform.

### 4.2 ONYX operational desktop

`App.tsx` loads `get_current_session` over Tauri IPC before mounting the authenticated routes. When it gets a stored session for an endpoint other than the canonical Worker address, it invokes logout and returns to the unauthenticated flow. A session-loading state is rendered during the lookup. A startup failure renders an error with retry. A null session renders the Login component outside React Router. A valid session mounts `MainLayout` and the session provider.

| Screen ID | Component / state | Route or trigger | Data/actions established in source |
|---|---|---|---|
| DESK-SCR-01 | Dashboard | `/` | Read sync status and display overview/links to missions, tasks and approvals |
| DESK-SCR-02 | Missions | `/missions`, `/missions/:missionId` | `GetMission` query; mission-ID lookup; `CreateMission` command with name/description |
| DESK-SCR-03 | Tasks | `/tasks`, `/tasks/:taskId` | `GetTask`; task-ID lookup; `CreateTask`; `MarkReady` action |
| DESK-SCR-04 | Approvals | `/approvals` | Lookup submitted task and open review dialog; approve/reject UI is present |
| DESK-SCR-05 | Messaging | `/messaging`, `/messaging/:conversationId` | Conversation list/detail, create conversation, membership changes, messages, edits/reactions/redaction paths, connections panel |
| DESK-SCR-06 | Files | `/files`, `/files/:fileAssetId` | `GetFileAsset`; upload/download through Rust; grant/revoke access; quarantine/archive actions; version listing |
| DESK-SCR-07 | Notifications | `/notifications` | Query notification inbox, subscribe to local Tauri event stream, acknowledge notification |
| DESK-SCR-08 | Settings | `/settings` | Account/settings view with reauthentication/logout callback |
| DESK-SCR-09 | Login | Returned by root App when session is null; not a router URL | Username/password fields; calls Rust login command; no separate `/login` route |
| DESK-ST-01 | Session loading | Startup before session resolved | Loading message; no authenticated page yet |
| DESK-ST-02 | Startup error | `get_current_session` IPC call rejects | Error title/message and retry control |
| DESK-DLG-01 | Approval review dialog | Opened from Approvals review action | Textarea; cancel/reject/approve controls; command invoked by the dialog component |
| DESK-OVR-01 | Accessible dialog wrapper | Component `AccessibleDialog` | Shared in-window dialog infrastructure; exact rendered usages not exhaustively enumerated |

There are **12 explicit operational route declarations** plus a catch-all redirect in `App.tsx`; the Login component is selected by root session state rather than a URL route. Mission/task/file/messaging detail routes reuse their corresponding page component and change the selected ID/state. They are not separate page modules, but their data state differs. `MainLayout` has eight primary navigation entries: Overview, Missions, Tasks, Approvals, Messaging, Files, Notifications, and Settings. It checks viewport width at `max-width: 960px` and has a mobile-width menu/drawer state; that responsive behavior exists in the desktop code and is not the separate Android or PWA UI.

### 4.3 ONYX Admin desktop

The Admin route table has eight route declarations including the catch-all: `/login`, index, `/users`, `/settings`, `/settings/:policyId`, `/profiles`, `/organizations`, and `*`. Index and `/users` both mount the Users page; settings with and without a policy ID reuse the Settings page. The principal page modules are:

| Screen ID | Component | Route | Visible purpose evidenced by source |
|---|---|---|---|
| ADM-SCR-01 | Login | `/login` | Username/password login through native login bridge |
| ADM-SCR-02 | Users | index and `/users` | User directory, account creation, class/parent and active-status operations |
| ADM-SCR-03 | Settings | `/settings`, `/settings/:policyId` | Organization policy/mobile access, policy lookup/create, policy rule/version operations |
| ADM-SCR-04 | Profiles | `/profiles` | Profile list/edit/create plus CSV/JSON import/export |
| ADM-SCR-05 | Organizations | `/organizations` | Organization list/create/rename/activate/deactivate and organization-admin management |
| ADM-ST-01 | Admin loading | Lazy route Suspense fallback | Loading text while page chunks load |
| ADM-ST-02 | Access denied | Authenticated user lacks `is_admin` | Access-required message and logout button |

`ProtectedLayout` requires an authenticated session. `AdminOnlyLayout` checks the response's `is_admin` field for a clear UI gate, but its own comment notes that the client guard is not a security boundary; Worker authorization is expected to enforce real authority. `AllFatherOrganizationsRoute` checks `role === "ALL_FATHER"` in the client navigation/route and otherwise redirects to `/users`; the Worker is the enforcement boundary. The navigation includes Users, Policy Settings, and Staff Profiles for standard administrators; Organizations is shown only for `ALL_FATHER`.

## 5. Static UI counts and what the numbers mean

These are counts of literal JSX tags in the specifically inspected page/layout/dialog source modules, **not** counts of every rendered runtime instance. They are reproducible static source counts; they exclude children repeated by `.map()`, values returned by imported/shared components not included in the counted file set, native OS chrome, and variants not rendered in a given state. Consequently, they do not satisfy the prompt's required exhaustive per-state rendered-instance inventory.

| Count type | ONYX desktop inspected source | Admin desktop inspected source | Notes |
|---|---:|---:|---|
| `<button>` JSX occurrences | 39 | 29 | Literal tags across inspected source modules; conditional visibility not reconciled |
| `<input>` JSX occurrences | 22 | 22 | Repeated reusable `Field` and loop controls mean runtime inputs can be more numerous |
| `<textarea>` occurrences | 3 | 0 | Missions, Tasks, ApprovalDialog in ONYX |
| `<select>` occurrences | 1 | 4 | Messaging conversation type; Admin role/class/rule/access contexts |
| Native `<form>` tags | 1 | 3 | Many source forms are visual `div`/`section` containers with button handlers, not `<form>` tags |
| `<table>` tags | 0 | 3 | Admin Users, Profiles, Organizations source pages |
| Primary route declarations including fallback | 13 | 8 | Login in ONYX is a session conditional, not a path route |
| Principal page components | 9 | 5 | Does not include transient states and shared dialog/layout components |
| Configured Tauri top-level windows | 1 | 1 | Config only; not runtime-observed |

Button/input totals are **source-structure totals**. They must not be represented as final design-review counts of all visible controls. The `screen-inventory.csv` includes principal screens/states. `interactive-elements.csv` captures confirmed source-level controls and repeated-component caveats; exhaustive control-instance counting is unresolved. See `counts-and-methodology.md` and `unresolved-facts.md`.

## 6. Forms and data operations observed in desktop source

### 6.1 Operational desktop

| Form/workflow ID | Fields / controls seen | Validation/guard seen in source | Handler/data path | Status |
|---|---|---|---|---|
| F-DESK-LOGIN | Username, password, submit | Username/password HTML `required`; button disabled while loading | `Login.tsx` -> Tauri `login` command -> Rust session authentication -> Worker `/api/auth/login`; tokens held natively | Source-confirmed; live auth unverified |
| F-DESK-MISSION | Mission name, description; separate 16-byte ID lookup field | Create disabled when name is empty; query ID parse handled by lookup helper | `useCommand` -> `invoke("execute_command")` with `CreateMission`; lookup -> `execute_query`/`GetMission` | Source-confirmed, not E2E tested |
| F-DESK-TASK | Task title, description, mission ID; separate task-ID lookup | Create disabled for blank title; mission ID parsed as JSON 16-byte array or a generated ID is used when blank | `CreateTask`; query `GetTask`; `MarkReady` | Source-confirmed, not E2E tested |
| F-DESK-APPROVAL | Target task ID; review dialog textarea | Lookup parses ID; only submitted task shown as review candidate per copy/logic | Query task and invoke approve/reject in review dialog | Source-confirmed; authority/data path not runtime tested |
| F-DESK-MESSAGE | Conversation ID, type selector, optional parent Supergroup ID for SubTeam, member ID, message body, edit body, reaction code, message ID/lookup, connection fields | SubTeam requires parent Supergroup ID parseable as a JSON array of 16 numbers; message posting rejects blank body in client | Local Rust command/query path via `useCommand`/`useQuery` | Source-confirmed; multi-client messaging not verified |
| F-DESK-FILES | File asset ID, local file path, download destination, target user ID | Upload button needs nonblank path; client constant max size 100 MiB; user ID must parse as 16-byte JSON array | `upload_file`/`download_file` Tauri commands; file access/quarantine/archive commands via command registry | Source-confirmed; cross-device persistence not verified |
| F-DESK-NOTIFY | No data-entry form; refresh and acknowledge controls | Loading/error/empty state and acknowledging state | local event subscription plus notification query and `Acknowledge` command | Source-confirmed; OS-native notification delivery not established |
| F-DESK-SETTINGS | Logout/re-authentication action | Logout state/error handling in MainLayout | Rust `logout` command | Source-confirmed |

### 6.2 Admin desktop

| Form/workflow ID | Fields / controls seen | Validation/guard seen in source | Handler/data path | Status |
|---|---|---|---|---|
| F-ADM-LOGIN | Username, password | Both `required`; submit disabled while loading | UI native-login adapter -> Tauri `admin_login` (Rust reqwest) -> canonical Worker `/api/auth/login` | Source-confirmed; production login unverified |
| F-ADM-USER | Username, contact email, initial password, operational class selector | Non-empty username/email/password required by disabled state; email `type=email`; class optional | `POST /api/admin/users`; user actions update status/class/parent with separate endpoint calls | Source-confirmed; endpoint contract documents Worker tenant/role boundary |
| F-ADM-POLICY | Policy name; rule type, rule key and value per rule; add-rule/save-draft actions; policy ID lookup | Empty name rejected; rule list validation present in page logic; uses operation ID/idempotency handling in create path | Worker `/api/admin/policies`, generic `/api/command` for policy version/rules, `/api/query` to inspect policies | Source-confirmed; no runtime verification |
| F-ADM-MOBILE-ACCESS | Five class checkboxes: top-level manager, senior manager, team leader, supervisor, staff | API response shape/version/enforcement-status validated; saves full list with expected version; on failure attempts reload of canonical server state | `GET`/`PUT /api/admin/mobile-access` | Source-confirmed; actual live policy state not checked |
| F-ADM-PROFILE-EDIT | Owner UUID, full name, job title, department, contact email, contact phone, class label, parent display name, photo blob key, team memberships | Client checks required `owner_id`, `full_name`, `contact_email`, `contact_phone`, `job_title`, `department` before save; some optional values serialize as null; semicolon list for memberships | `PUT /api/admin/profiles` | Source-confirmed; data model semantics partly ambiguous |
| F-ADM-PROFILE-TRANSFER | File picker and CSV/JSON export actions | Accepts `.csv`/`.json` and matching MIME types; summary includes created/updated/failed rows | `POST /api/admin/profiles/import`; `GET /api/admin/profiles/export` | Source-confirmed; XLS/XLSX not accepted by this UI |
| F-ADM-ORGANIZATION | Organization name create/rename; select an organization; create admin username/email/password; deactivate/reactivate org/admin | Name required and max length 160 in visible input; admin fields required; password min length 8; operations disabled while busy/in inactive org conditions | `/api/allfather/organizations`, `/api/allfather/admins`, related activate/deactivate endpoints | Source-confirmed; requires canonical All-Father Worker principal |

Source field-level detail is incomplete for every optional/conditional control and every server validator. The per-field CSV is a traceable, confirmed-minimum inventory; it does not claim that uninspected runtime validations are absent.

## 7. Desktop UI → Rust integration

The operational desktop frontend uses Tauri 2 IPC and calls the native command handlers registered in `crates/bins/desktop-shell/src/lib.rs`. Relevant registered handlers include `execute_command`, `execute_query`, `subscribe_events`, `get_sync_status`, `upload_file`, `download_file`, `store_secret`, `get_secret`, `delete_secret`, `login`, `logout`, and `get_current_session`. The frontend's `useCommand` constructs a `CommandEnvelope` and calls `invoke("execute_command")`; `useQuery` calls `invoke("execute_query")`. `subscribe_events` forwards local domain events to the webview as `onyx:event`. `MainLayout` polls sync status every five seconds.

The native session module posts a username/password to `{server_address}/api/auth/login`, stores the returned access/refresh token pair in a serialized session using the `SecureStorage` port, reloads it at startup, and deletes it on logout. The desktop application constructs this store with `KeyringSecureStorage`. The UI-facing `SessionInfo` excludes raw tokens; session IDs and actor/device IDs cross IPC. The root App additionally checks that saved sessions are bound to the single canonical Worker origin.

**Observed architectural distinction:** successful login is a remote Worker request, while the operational desktop `useCommand` and `useQuery` calls dispatch locally to the native Rust `AppState`'s command/query registries and local repositories. The project also configures synchronization/relay infrastructure. The available evidence does not establish end-to-end remote replication/consistency for each individual command during this audit. The architecture guide likewise says Worker/D1 and native Axum/SQLx runtimes have separate deployment/persistence configurations; do not assume shared state.

Admin UI uses a different path. `crates/bins/admin-shell/src/login.rs` defines a native `admin_login` Tauri command, a canonical Worker base URL, reqwest, and a 30-second login timeout. After login, Admin page API client calls Worker endpoints directly. `crates/bins/admin-shell/ui/src/utils/auth.ts` stores the Admin access token, refresh token, and user record in `sessionStorage`; its store bootstraps from that storage. Code inspected does not prove durable, operating-system-keyring persistence across Admin app restarts. This differs from the explicit keyring-backed session persistence implemented in `desktop-shell`.

## 8. Current backend, schema, and capability inventory

### 8.1 Worker API contract

The source-level `docs/production/api-contract.md` contains **47 endpoint rows**. They cover health/readiness; login/refresh/logout; All-Father organization/admin operations; query/command; events WebSocket and scoped relay tickets; users/hierarchy and Admin user changes; mobile-access policy; policy/legal-hold creation; staff profiles and CSV/JSON import/export; mobile todo/target/staff-loan aggregates; push subscriptions; file downloads and file ACL. This is a contract inventory, not a claim that every route is live or that every complex workflow passes E2E. It also documents `501 ROUTE_NOT_MIGRATED` as a possible response for paths lacking a migrated implementation. [API contract](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/production/api-contract.md#L1-L77)

### 8.2 Data persistence surfaces

| Data resource | Evidence and interpretation | Confidence |
|---|---|---|
| Worker `users` | D1 users contain identity, organization membership, active/admin state, class, parent/reporting linkage, and role columns across migrations | High |
| Worker `organizations` | D1 organization entity with active state/name/timestamps; canonical system organization `__allfather__` | High |
| `aggregates`, `domain_events`, `idempotency` | Worker D1 schema includes operational aggregate/event/idempotency persistence | High |
| `token_revocations`, `user_session_revocations` | Token and user-session revocation tables exist in migration/source | High |
| `replica_ownership`, `relay_ticket_redemptions` | Ownership binding and one-time relay ticket redemption persistence exists | High |
| `file_assets`, `audit_log`, `push_subscriptions` | Defined by Worker foundation migration | High |
| Admin profile fields | `owner_id`, `full_name`, `job_title`, `contact_email`, `contact_phone`, `department`, `photo_blob_key`, `class_label`, `parent_display_name`, `team_memberships` appear in UI transfer model | High for UI schema; database normalization/cardinality not fully audited |
| Department/team entities | No dedicated `departments` or `teams` D1 tables found in inspected migration set; department and team membership also appear as profile values | Medium; full schema sweep not exhaustive |
| Timesheet/payment records | Not found as current API routes, domain crates, or migration entities in the inspected repository areas | Medium; bounded absence finding |

### 8.3 Capability status register

Statuses are based on code path and evidence, not an assumption that the deployed app works.

| Capability ID | Capability | Current evidence | Status |
|---|---|---|---|
| CAP-AUTH-01 | ONYX username/password authentication | Worker `POST /api/auth/login`, salted PHC Argon2id, token pair, session revalidation | Implemented, not production E2E verified |
| CAP-AUTH-02 | All-Father account authority | Canonical D1 principal/role/admin/active checks and server-side All-Father routes | Implemented; live provisioning/auth remains unverified |
| CAP-ORG-01 | Create/list/rename/activate/deactivate organizations | Admin Organizations page and Worker endpoints; `ALL_FATHER` gates | Implemented, not live E2E verified |
| CAP-ADMIN-01 | Create/manage org administrator accounts | Admin Organizations page/Worker endpoint contract | Implemented, not live E2E verified |
| CAP-USER-01 | Staff/user management | Admin users page; tenant-scoped APIs for create/activate/deactivate/password/class/parent/manager | Implemented, not full end-to-end verified |
| CAP-HIER-01 | Parent/reporting hierarchy | `parent_user_id`, hierarchy endpoint, cycle error in Axum admin source; separate Worker/Native backend boundaries | Partially implemented across code paths; parity not verified |
| CAP-MISSION-01 | Mission create/query/lifecycle | Desktop page + Rust domain and generic API/query contracts; Worker command policy | Implemented in source, end-to-end persistence/sync not verified |
| CAP-TASK-01 | Task create/mark-ready/lifecycle | Desktop page + Rust domain; Worker policy enumerates task operations | Implemented in source, end-to-end persistence/sync not verified |
| CAP-APPROVAL-01 | Task/mission approval | Approval UI and Worker command policy entries; native domain transitions | Implemented in source, cross-runtime E2E not verified |
| CAP-MSG-01 | Conversation/message lifecycle | Communication domain and desktop UI for Direct/Channel/Supergroup/SubTeam, messages, reactions and redaction paths | Implemented in source; client parity and end-to-end operation not verified |
| CAP-FILE-01 | File assets, versions and ACL | file-domain, desktop local path upload/download, Worker file/ACL endpoints, 100 MiB UI constant | Implemented in source; remote storage/relay end-to-end not verified |
| CAP-NOTIFY-01 | Notification list/acknowledgment | Desktop notification aggregate query/event stream; Worker push subscription endpoints | Implemented in source; OS delivery/realtime E2E not verified |
| CAP-POLICY-01 | Policy create and version/rule editing | Admin Settings page + Worker policy API/generic command path | Implemented in source; live validation not verified |
| CAP-MOBILE-01 | Class-based mobile access policy | five-value class policy, version/concurrency and enforcement-status logic | Implemented in source; live enforcement not verified |
| CAP-PROFILE-01 | Staff profiles and bulk transfer | Admin page, CSV/JSON transfer and Worker API | Implemented in source; no XLS/XLSX evidence |
| CAP-TODO-01 | Todo/target/staff-loan mobile aggregates | Worker contract and generic API route families | Implemented in source-level contract; platform workflow not E2E verified |
| CAP-RELAY-01 | Scoped relay transport | Worker issues 30-second relay ticket and checks target, expiry and one-time redemption; Durable Object hub | Implemented in source; live network behavior not verified |
| CAP-TIME-01 | Timesheets | No corresponding current module/route/table found in inspected sources | Not found in inspected sources |
| CAP-PAY-01 | Payments | No corresponding current module/route/table found in inspected sources | Not found in inspected sources |
| CAP-MOD-01 | Optional first-party business-module activation/deactivation | Cargo domain crates and services are modular in code; no complete product-level optional-module registry/install/activation scheme found | Unresolved; modular code organization is not proof of module packaging |
| CAP-SPIRAL-01 | Interactive spiral org/navigation visualization | No current spiral-specific source or dependency evidence found by source search | Historically recalled; current implementation unresolved |

## 9. Roles and authorization

The Worker currently declares the authority roles `ALL_FATHER`, `ORGANIZATION_ADMIN`, and `STAFF` in `authorization.ts`. Operational classes are separate values; the current five values exposed by Admin/mobile-access UI are `top_level_manager`, `senior_manager`, `team_leader`, `supervisor`, and `staff`. A legacy `is_manager` flag and `parent_user_id` reporting relationship also exist. `is_admin`, account `role`, operational `class`, and reporting parent are not interchangeable concepts.

| Role / class | UI and API capabilities evidenced | Enforcement evidence / limits |
|---|---|---|
| `ALL_FATHER` role | Organization creation/list/rename/activation; create/admin-account management; Organization route in Admin | Worker verifies canonical principal, role, admin flag, active user/org and checks All-Father routes server-side. UI-only gating is not the authority boundary. |
| `ORGANIZATION_ADMIN` role | Tenant-scoped staff account management, policy and legal-hold APIs, mobile-access setting, profile import/export, file ACL; Admin navigation for Users/Settings/Profiles | Worker contract and route/policy source enforce role and own-tenant constraints for listed endpoints. Full per-route runtime test was not run. |
| `STAFF` role | Regular ONYX login, own records/limited operational task and notification actions, profile access subject to ownership | Worker command policies explicitly restrict many staff updates to owned records; endpoint and command-specific behavior must be read individually. |
| `top_level_manager`, `senior_manager`, `team_leader`, `supervisor`, `staff` classes | User class metadata and mobile capability allowlist | Used in mobile capability policy, not a substitute for `role`. The exact set of operations available to each class depends on route and server-side policy. |
| `is_manager` | Legacy/compatibility manager field; user administration endpoint can update it | It is not one of the three Worker role values; no inference of All-Father/org-admin authority follows from this flag. |
| HR / general manager | No dedicated Worker role named `HR` or `GENERAL_MANAGER` found in inspected role enum | Similar business titles may be represented as `class`, but that is not evidence of a separate HR role or a specific HR permission set. |

`authorization.ts` uses explicit command policies and describes unlisted actions as denied. The Worker `requireSession` path queries current user/organization status, role and revocation information instead of treating client token metadata as permanently authoritative. By contrast, old `docs/DECISIONS.md` increments include a historical stub `VerifiedAuthority::is_authorized` that unconditionally returned true in an early domain increment; that old native-domain note is not evidence that the current Worker guard behaves the same way. Backend responsibilities are separate, so parity must not be assumed.

## 10. Organizational structure, diagram, and spreadsheet evidence

### 10.1 Entities and relationships found

- **Organization:** Worker D1 `organizations` table; an organization has an ID, name, active state and timestamps. Organization records are operated through All-Father-scoped UI/API paths.
- **User/principal:** Worker D1 `users`; fields visible across migrations and Worker reads include `id`, `username`, `email`, `organization_id`, `password_hash`, `is_admin`, `is_active`, `class`, `role`, and `parent_user_id`; a legacy manager flag is also present. User is joined to an active organization during Worker authentication.
- **Reporting relationship:** `parent_user_id` is used for the reporting tree; current API contract includes `GET /api/users/hierarchy`. Native Axum admin source includes a `ParentNotFound` error and `ParentCycle` error. Worker and Axum use distinct runtime/data boundaries; the exact parity and cycle enforcement in the deployed Worker were not runtime tested.
- **Operational class:** fixed named metadata used in account editing and mobile access policy; values include top-level manager, senior manager, team leader, supervisor and staff. Exact meaning differs from account role.
- **Staff profile:** UI/transfer schema includes required owner ID and fields for full name, contact email/phone, job title and department, with optional class label, parent display name, photo blob key, and `team_memberships` as semicolon-separated UI text mapped to an array. This profile data duplicates/describes people and org relationships; a canonical department or team entity has not been established by the inspected D1 migration evidence.
- **Audit event:** Worker foundation migration creates `audit_log`, with organization and user IDs and creation time evidence. Full event taxonomy and retention policy were not exhaustively traced.

### 10.2 Support matrix for organizational hierarchy

| Requirement | Classification | Basis |
|---|---|---|
| Organization contains user accounts | Explicitly supported | `users.organization_id`, active-org join, organization APIs |
| User can have one reporting parent | Explicitly/partially supported | `parent_user_id`, hierarchy endpoint, parent mutation, cycle error in native API |
| Manager supervises multiple users | Partially supported | Multiple children may refer to one parent in a parent-pointer model; cardinality behavior and UI rendering not runtime tested |
| Dedicated department entity/table | Not found in inspected sources | `department` appears as staff profile field; no dedicated table surfaced in inspected migration set |
| Dedicated team entity/table | Not found in inspected sources | `team_memberships` appears as profile field/array; no dedicated table surfaced in inspected migration set |
| User belongs to multiple teams | Partially represented | `team_memberships` can contain multiple strings in UI schema; authoritative relation table/cardinality not established |
| Temporary reporting relationship | Not found in inspected sources | No effective-from/effective-to reporting fields found in inspected evidence |
| Vacant positions | Not found in inspected sources | No position/seat entity in inspected data model evidence |
| Cross-department reporting | Undetermined | Department is profile metadata rather than an evidenced normalized hierarchy entity |
| Circular hierarchy rejection | Partially supported | Native Axum has `PARENT_CYCLE`; Worker contract says parent cycle checked; deployment was not exercised |
| Interactive organization tree/graph/spiral | Not found in inspected sources | No dedicated chart/tree/graph or spiral implementation identified in inspected desktop source/dependency manifests |

### 10.3 Spreadsheet and chart libraries

`Profiles.tsx` and `profileTransfer.ts` support profile import/export in **CSV and JSON**. The UI accepts `.csv` and `.json`, checks required profile columns, reports per-row created/updated/failed outcomes, and exports CSV or JSON. This is profile batch transfer, not evidence of importing organization charts or Excel workbooks. No `.xlsx` or `.xls` support was found in the inspected profile-transfer source. The `web-ui` manifest includes Recharts, but that manifest evidence does not establish an organizational hierarchy chart in the two desktop apps. No dedicated graph/spiral library or custom node-edge chart implementation was identified in the inspected desktop UI source tree.

## 11. Cross-platform responsibilities: desktop, Android, and PWA

The Desktop UI is the primary local operational workspace in the current code layout, but the historical “desktop-first” operating model was not found as a complete formal policy in current documentation. The desktop app provides mission/task creation, approvals, messaging, file access controls, notification acknowledgements, and sync status. Its screen and command model is a substantial local-first operational surface.

The native Android app is Kotlin UI over Rust mobile core/JNI and uses ONYX credential login/session state; inspected Android shell source wires create-mission and create-task actions and renders sync/conflict information. That means the native Android source is not limited to passive alert/overview-only UI. Its successful end-to-end write/sync behavior was not tested here. The Android manifest declares network, Wi-Fi Direct, Bluetooth/BLE-related permissions and labels both peer-to-peer mediums optional at install level; using these transports on a real device is still a runtime question.

The `mobile-pwa/package.json` description calls the product the “ONYX ObserverClient PWA” and describes it as a read-only browser client for the `mobile_observer` client class. It declares build/test/type-check/lint and Playwright scripts, but package scripts do not prove the workflows passed in this audit. The web UI also exists as a separate React SPA and is in scope for the full product audit, but this report does not attempt to enumerate all browser-only screens because the first prompt explicitly excludes those surfaces from the desktop UI inventory.

| Client | Operation evidence | Responsibility status |
|---|---|---|
| Windows/Linux desktop | Mission/task/messaging/file/approval/notification source; local Rust command/query; Worker login | Operational client source-confirmed; Windows/Linux E2E unverified |
| Admin desktop | Organization/admin/user/policy/profile management; Worker APIs | Privileged administration source-confirmed; live All-Father access unverified |
| Android | Login/session storage, dashboard, missions/tasks, sync/conflicts, optional P2P transports | Contains both information and write-action source; end-to-end mobile sync unverified |
| PWA | Read-only observer description and client-class access model | Intended/read-only role documented; runtime behaviour unverified |
| Web UI | Separate app; package manifest includes dashboards/charting library and tests | Detailed capability parity not established in this report |

The difference between native Android's source-level write actions and the remembered “Android is mainly alerts, emergencies, and overviews” principle is **unresolved**: current source disproves a strict “mobile is read-only” statement, but does not by itself prove the original operating model was formally reversed. The PWA is explicitly read-only by package description, while native Android is not presented that way.

## 12. Product vision, modularity, and historical chronology

### 12.1 Product identity

The current repository description identifies ONYX as a “Mission Operations Platform”; the README's positioning emphasizes a connected workspace for organizational work. Current mission/task domain models, approval workflows, staff profiles and policy functions support that general operational purpose. The audit did **not** find primary-source confirmation in current `main` for the exact recalled phrases **“Tech to Select”** or **“Modular Managing Platform.”** They may belong to earlier unavailable context or a prior state; present evidence cannot determine this. The same applies to the recalled **spiral** concept as a historical design intent.

### 12.2 Modularity

The Rust code is modular at the implementation layer: the workspace has 43 members, including eight domain crates (`communication-domain`, `file-domain`, `mission-domain`, `notification-domain`, `policy-domain`, `profile-domain`, `todo-domain`, `work-domain`), five application crates, and six binary crates. These counts describe code organization. They do not demonstrate a runtime module registry, optional feature installation, license-based module activation, dynamic module marketplace, or end-user install/uninstall mechanism.

The Worker has per-organization mobile-class access policy, version/concurrency state, and class-to-capability enforcement. That is a feature policy, not proof that entire business modules such as Timesheets or Payments can be independently activated/deactivated. The repository has no inspected timesheet/payment routes or entity schemas. Messaging, file assets and relay transport appear in source, but their optional packaging relationship is unproven. Third-party extensions are not evidenced by a current plugin/extension SDK in the inspected areas.

### 12.3 Major evidence-backed history

| Date / event | Source | Finding and confidence |
|---|---|---|
| 2026-08-07 repository created (metadata) | GitHub repository metadata | Current repository history begins on this date; does not establish the original product's full history before GitHub |
| 2026-08-14/15 Admin platform recorded as requested/completed | root `DECISIONS.md`, Admin Platform entries | A separate Admin platform was an explicit decision; current separate Admin shell matches it. High for current decision-log claim; medium for full original context |
| Flutter v1.1 deferred in earlier increment notes | `docs/DECISIONS.md` around §FS-R4 / M11 | Earlier records describe Flutter app as deferred; later current history changes this position. High that the doc records the earlier plan |
| Flutter retirement override recorded; legacy `mobile/` removed | `docs/DECISIONS.md` at FLT-1 and current `docs/Structure.md` | A later owner decision explicitly says its retirement gate was not met but deletion proceeded; current source inventory confirms `mobile/` absent. High |
| 2026-10-18 PR #267 merged | GitHub PR #267 | Replaced All-Father Google login path with ONYX credentials; current auth docs/source align. High |
| 2026-10-18 PR #268 merged | GitHub PR #268 | AuthZ/session hardening in history; current Worker `requireSession`/explicit role checks support the present enforcement model. High for merge/source facts |
| 2026-10-24 PR #295 merged | GitHub PR #295 | CI scope reduced for demo; performance/load tests removed from current core gate as out of current demo scope. High |
| 2026-10-24 PR #303 merged | GitHub PR #303 | Production provisioning changed to D1 REST query endpoint; PR description/history does not establish a successful production password update. High |
| 2026-10-24 PR #304 merged | GitHub PR #304 | Admin login is sent from Rust rather than WebView; current Admin login Rust handler supports this boundary. CI on current main does not independently prove real Windows/Linux login click-through |
| 2026-10-25 PR #325 merged | Current main merge commit | Added cache-purge guide following CI audit; the accompanying completion report says stale caches still required privileged deletion at the time of the report. High |

### 12.4 Licensing

`LICENSE.md` explicitly calls ONYX proprietary. Cargo metadata references that file and disables publication. GitHub's `NOASSERTION` reflects the absence of a standard SPDX ID in metadata; it is not evidence of MIT or Apache licensing. This audit makes no licensing change. The exact commercial grant, exceptions for third-party dependencies, or any separately executed contract beyond the repository notice were not audited beyond accessible repo text.

## 13. CI and demonstration readiness

### 13.1 Recorded automation

- Current main CI run **40198813416**: completed **success** on the audited revision (GitHub run metadata). This is current main-branch CI evidence.
- CI completion report's referenced required jobs for an earlier PR run 40060958512 include Rust/check, security, i18n, web, mobile-PWA, native UI evidence, source contracts, and merge gate, all recorded success in that report. Separate Cloudflare Worker CI and All-Father local auth smoke are recorded success for PR #298-era run IDs. These are historical CI records, not a substitute for each current runtime scenario.
- The current completion report explicitly notes manual “Provision All-Father Password” workflow failures and says live production provisioning was not done; it also notes cache deletion required repository-admin permissions. Current auth documentation independently says no production DB/service was modified by the code change and live All-Father authentication remains unverified.
- PR #295 removed demo-out-of-scope performance/load tests and consolidated CI. Performance acceptance/load-test coverage should not be represented as a current green CI gate.

### 13.2 Demo workflow status

| Demo scenario | Evidence-backed setup / expected path | Current readiness statement |
|---|---|---|
| Sign into ONYX desktop | Canonical Worker endpoint, existing active user/org, valid ONYX credentials; UI invokes Rust login and stores tokens in OS secure storage | Source path exists; not clicked through on Windows/Linux in this audit |
| Show mission/task overview and commands | Desktop shell with valid user and local app-state database/sync setup | Source and generic command/query paths exist; end-to-end data creation/refresh and cross-runtime persistence unverified |
| Show approvals | Use a submitted task visible through local query source; use review dialog | UI and commands exist; approval lifecycle E2E unverified |
| Show messaging | Valid organization IDs, conversation/message data or creation path | UI/domain code exists; remote/mobile sync and live persistence unverified |
| Show file upload/download | Local path accessible to the desktop process; permissions and storage adapter available; 100 MiB UI limit | UI/Rust actions exist; actual byte persistence/remote retrieval not tested |
| Show notifications and sync | Local event bus, notification state, sync agent | Local query/event hooks exist; OS push delivery not proven |
| Sign in to Admin | Active organization admin / canonical All-Father, correct D1 account state and Worker URL | Source path exists; account/session and production state must be treated as unverified |
| Create organization and org admin | Canonical active All-Father principal with `ALL_FATHER` role and admin flag in Worker D1 | UI and Worker routes exist; live permission operation not tested; must not claim successful live demonstration before credentials/provisioning verified by owner |
| Show profiles CSV import/export | Org Admin, valid CSV/JSON, existing user IDs and required profile columns | Source logic exists; CSV/JSON supported, not Excel; live backend not tested |
| Show mobile observer | PWA deployment and authorized observer account/class policy | Documentation/manifests identify observer-client/read-only role; live deployment and role restrictions unverified |
| Show Android sync/P2P | Android build/install, valid login, required native permissions/device support and remote/local synchronization | Kotlin/JNI and P2P code exist; physical-device/runtime transport success unverified |

**Overall demonstration conclusion:** source and CI evidence show a substantial demo-shaped product, but this audit cannot certify a live All-Father login, Windows/Linux click-through, or cross-platform data consistency. The most consequential uncertainty for an All-Father-led demo is the difference between a successful isolated local smoke and the currently unverified deployed D1 account state. This is an observed risk, not a probability estimate; no statistically grounded success probability can be derived from the evidence inspected.

## 14. Windows/Linux differences, assets, motion, accessibility

Both Tauri configs identify one window at 1280×800 and use `bundle.targets = "all"`. ONYX defines icon formats `icns`, `ico`, and PNG, plus Linux `.deb` Hicolor icon mappings from 16 to 512 directory size labels. Admin has the analogous bundle/icon configuration and an explicit `admin-main` Tauri capability declaration. The source configs do not set min dimensions or a `resizable` value. No native menu, system-tray handler, file-picker plugin flow, clipboard integration, or Windows/Linux conditional UI implementation was established in the selected files. OS-specific file-system activity exists in Rust upload/download commands; dialog/native behavior outside inspected source remains unresolved.

The desktop CSS defines the light operational palette through `@theme`, including background `#f3f7fb`, white surface, surface-hover `#e8f0f8`, border `#d8e2ec`, text `#102b4d`, dim text `#5d7088`, accent `#1f6098`, and status colors for draft/active/review/approved/blocked/closed. The body uses an Inter/system font fallback stack; no bundled desktop font file was confirmed. CSS contains a nav transition of 160 ms/ease across background-color, border-color, color and transform; selected components use Tailwind transition utilities. No keyframe animation definition was found in the inspected desktop CSS. Reduced-motion behavior and every hover/focus/active transition were not exhaustively inventoried or runtime-tested.

The operational CSS uses visible focus outlines for buttons, links, inputs, textareas and selects. `AccessibleDialog.tsx` indicates shared dialog accessibility infrastructure and `MainLayout` manages keyboard/drawer state. This is source evidence of accessibility mechanisms, not proof of complete keyboard, screen-reader, reduced-motion, or Windows/Linux parity.

## 15. Discrepancy, risk, and uncertainty register

| ID | Observed discrepancy/uncertainty | Evidence and consequence | Confidence |
|---|---|---|---|
| GAP-01 | Current documentation says live All-Father production auth is unverified; local smoke passed in recorded history | A demo login cannot be classified as production-verified based on local D1 smoke alone | High |
| GAP-02 | Login and operational command data paths are not one backend tier | Desktop login calls Worker, while command/query UI dispatches to local native `AppState`; Worker/D1 and SQLx runtimes are documented as separate | High for architectural separation; end-to-end replication unknown |
| GAP-03 | Admin token persistence differs from operational desktop | Admin auth helper uses `sessionStorage`; operational desktop uses Rust secure-storage/Keyring adapter | High for source locations; cross-restart behavior untested |
| GAP-04 | Admin shell Cargo description says direct API-server HTTP while current auth handler uses canonical Worker URL | Product/package description appears stale or oversimplified; authoritative live route use is not verified by deployment test | High for observed text/code, medium for extent of contradiction |
| GAP-05 | Historical Flutter/iOS plans vs current source inventory | Old docs contain Flutter/iOS plan and some Rust platform FFI; current tree has no legacy `mobile/` app and docs record owner-overridden Flutter retirement | High |
| GAP-06 | Remembered exact brand phrases and spiral are not in inspected current source evidence | Historical project intent could predate current branch or exist in uninspected GitHub discussions | Medium |
| GAP-07 | Role labels vs operational class labels can be conflated | Worker has 3 authority roles; class is a separate five-value field; `is_manager` is another field | High |
| GAP-08 | Hierarchy metadata without dedicated org-unit schema | User parent relationship and profile `department`/`team_memberships` exist; dedicated department/team tables and chart module not found | Medium |
| GAP-09 | CSV/JSON profile transfer vs recalled Excel workflows | Profile uploader accepts CSV/JSON only; no XLS/XLSX handler found in inspected UI transfer code | High for present UI path |
| GAP-10 | Timesheets/payments/optional module packaging not found | Current domains/routes do not expose these as implemented modules in inspected source inventory | Medium |
| GAP-11 | Desktop “rendered element” counts cannot yet be exact | Conditional content, data-driven repeated controls, and shared imported children require instance-by-instance reconciliation and runtime states | High |
| GAP-12 | Windows/Linux behavior not run on both operating systems | Same config file and generic source do not prove packaged runtime behavior or OS-level integration | High |
| GAP-13 | `main` branch API metadata reports branch protection disabled | Current source can be checked by CI, but repository metadata does not establish a merge-protection rule for main | High for API snapshot; governance consequence outside this audit's changes |
| GAP-14 | Worker health/readiness not verified live by this audit | Browser tool could not access the explicit `/health` and `/ready` URLs; there was no direct runtime probe from another client | High |

## 16. Completion assessment against the attached prompts

### Prompt 1 — Desktop forensic inventory

**Completed from source:** exact revision, two desktop apps, two configured windows, route tables, main page/component inventory, major controls/forms and handler paths, Rust/Tauri integration boundary, principal design tokens, selected motion/keyboard patterns, current organization-data evidence, history and platform packaging configuration.

**Partially completed:** element-by-element register, form fields/validation completeness, every screen state, animations/hover/focus, asset dimensions and all uses, Windows/Linux behavior matrix, complete Tauri command usage, fully reconciled control-instance totals. Static source counts are intentionally marked as such.

**Not verified:** local builds/tests, Windows/Linux runtime, all keyboard shortcuts and OS services, rendered-instance count across all data states, live API response behavior, complete screenshots or click-through flows.

### Prompt 2 — Product knowledge/operational audit

**Completed from accessible evidence:** current product identity, current app/worker/runtime boundaries, authentication model, authority-role vs class model, high-level business capabilities, selected end-to-end source paths, organization hierarchy and spreadsheet evidence, Android/PWA distinctions, high-impact historical changes, CI/demo evidence and unresolved conflicts.

**Partially completed:** older Git history outside accessible current source and selected PRs, full GitHub discussions/issues/reviews, every Worker handler and native Axum route parity, complete data schema and relationship cardinality, every cross-platform workflow, runtime E2E proof, exact historical chronology before available repository records.

**Not verified:** live deployment status, production D1 state, All-Father credential acceptance, on-device Android behavior, packaged Windows/Linux behavior, cross-device/Worker/native persistence consistency, or the original spiral/brand narrative outside the checked repository evidence.

### Final handoff

The report uses current `main` revision `d8f64889cfe4473d40d50c41b0992bd22955eb7b`. Main CI run 40198813416 is recorded success. The static UI counts are source-code counts, not runtime-visible counts. The current repository has two Tauri desktop apps, 12 operational desktop routes plus fallback, 8 Admin route declarations including fallback, two configured 1280×800 desktop windows, nine operational page components and five Admin page components. The Worker API contract has 47 endpoint rows. The product-level source includes 43 Cargo workspace members and 8 domain crates. Live All-Father authentication, true remote persistence of desktop operations, complete hierarchy visualization, XLSX import, timesheets, payment module, optional module activation and historical spiral concept remain unverified or not found in inspected sources.

No source, config, credentials, database, deployment, or production resource was modified as part of the audit.

## 17. Primary evidence register

The companion `evidence-register.csv` records each major finding and its evidence link. Primary sources follow.

- [README.md](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/README.md)
- [Cargo workspace](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/Cargo.toml)
- [Repository structure](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/Structure.md#L5-L39)
- [Backend architecture](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/BACKEND_ARCHITECTURE.md#L3-L47)
- [Security/authentication model](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/SECURITY_AUTHENTICATION_MODEL.md#L1-L45)
- [Worker API contract](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/production/api-contract.md#L1-L77)
- [Worker route/authentication source](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/deploy/cloudflare/edge-gateway/src/index.ts#L147-L228)
- [Worker command authorization matrix](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/deploy/cloudflare/edge-gateway/src/authorization.ts#L8-L75)
- [Operational desktop routes](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/desktop-shell/ui/src/App.tsx#L25-L95)
- [Admin desktop routes](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/admin-shell/ui/src/App.tsx#L30-L90)
- [Tauri command registry and file operations](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/desktop-shell/src/lib.rs#L92-L355)
- [Operational desktop session persistence](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/desktop-shell/src/session.rs#L103-L197)
- [Admin user/profile/organization screens](https://github.com/SMozaff/Onyx-Framework/tree/d8f64889cfe4473d40d50c41b0992bd22955eb7b/crates/bins/admin-shell/ui/src/pages)
- [Flutter retirement record](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/docs/DECISIONS.md#L2203-L2282)
- [Current CI completion report](https://github.com/SMozaff/Onyx-Framework/blob/d8f64889cfe4473d40d50c41b0992bd22955eb7b/.github/COMPLETION_REPORT.md)
- [PR #267 — All-Father ONYX credentials](https://github.com/SMozaff/Onyx-Framework/pull/267)
- [PR #268 — authorization/session hardening](https://github.com/SMozaff/Onyx-Framework/pull/268)
- [PR #295 — demo CI consolidation](https://github.com/SMozaff/Onyx-Framework/pull/295)
- [PR #303 — D1 REST provisioning path](https://github.com/SMozaff/Onyx-Framework/pull/303)
- [PR #304 — native Admin login](https://github.com/SMozaff/Onyx-Framework/pull/304)
- [PR #325 — CI/cache audit docs](https://github.com/SMozaff/Onyx-Framework/pull/325)