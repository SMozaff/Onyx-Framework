# ONYX Mobile Client Migration Plan

**Document:** `docs/mobile-migration/MIGRATION_PLAN.md`  
**Date:** 2026-09-24  
**Revision:** Phase 0 update, 2026-09-23  
**Status:** PHASE 0 IN PROGRESS — BACKEND/JNI FACTS CORRECTED, CONTRACTS BEING FROZEN

## Summary

This plan outlines the migration from Flutter to a native Android Kotlin + iOS PWA architecture per ONYX-MOB-00 §25 and ONYX-MOB-01 §12–§26. The migration overrides the Flutter retirement gate (FLT-1) because the gate was unmet (JNI never run on real device, P2P transport stubbed with ConnectionLost placeholders, background/offline behavior unverified, no rollback artifact).

**Key locked decisions (from embedded questions):**
- P2P transport FFI direction: **Kotlin→JNI direction (Recommended)** — Kotlin owns `WifiP2pManager`/`BluetoothLeScanner`; Rust exposes JNI entry points for framing/encryption.
- Hardware: **Both Android and iPhone available** — Phase 4 (Android acceptance) and Phase 5 (PWA acceptance) can close on real hardware.
- PWA hosting: **To be decided later** — deferred to Phase 5 deployment work.

---

## Phase 0 — Contracts & docs

Phase 0 produces these frozen starting points:

- `docs/mobile-migration/flutter-retirement-record.md`
- `docs/mobile-migration/observer-capability-matrix.md`
- `docs/mobile-migration/observer-enforcement-inventory.md`
- `docs/mobile-migration/android-jni-contract.md`
- `docs/mobile-migration/pwa-observer-contract.md`
- `docs/mobile-migration/pwa-capability-matrix.md`
- `docs/DECISIONS.md` entry `P2P-1` for the locked Kotlin→JNI P2P direction
- Corrections to this plan and `KOTLIN_IMPLEMENTATION_PLAN.md`

### Phase 0.1 — Policy documentation

- `observer-capability-matrix.md`: server-owned `mobile_observer` ceiling, allowed/forbidden operation classes, deterministic denial shape, and refresh behavior.
- `flutter-retirement-record.md`: gate-overridden retirement record citing `FLT-1`.
- `observer-enforcement-inventory.md`: exact current backend coverage plus Phase 1 gaps.

### Phase 0.2 — Implementation contracts

- `android-jni-contract.md`: 15 JNI declarations with implementation status, marshalling rules, and open callback/secure-storage items.
- `pwa-observer-contract.md`: planned read-only ObserverClient/HTTP surface and missing backend prerequisites.
- `pwa-capability-matrix.md`: PWA-facing rendering of the server ceiling.
- `P2P-1`: locked Kotlin→JNI direction; update `KOTLIN_IMPLEMENTATION_PLAN.md` Layer 6 accordingly.

### Phase 0.3 — Runtime gates (Phase 0.4 includes CI verification)

**G001 — capability-negation-e2e**
- Mobile device test (Phase 4): Try every mutation (CreateMission, UpdateTask, Approve, TransitionLifecycle, ResolveConflict, UploadFile, UserMutation, OrgMutation, PolicyMutation, AdminCommand) while authenticated as `mobile_observer` — all must 403.

**G002 — pwa-observer-acceptance**
- iOS device test (Phase 5): `client_type="mobile_observer"` login, read every permitted projection, verify mutation endpoints return 403.

**G003 — security-audit**
- Run OWASP Top 10, CSP, session fixation, credential leakage scans against both clients.

---

## Phase 1 — Backend observer enforcement (P1)

**Pre-condition:** Observer capability contracts are in docs.

**Verified starting point:** `require_capability` already exists, `/api/command` and todo creation already deny observer domain submission, and `require_admin_mutation` already combines active-admin checks with `can_administer`. Existing `mobile_observer_capability.rs` tests already cover observer reads, representative command/todo/admin/profile/policy/legal-hold denials, and refresh preservation. Phase 1 is therefore completion and hardening, not initial enforcement.

### Phase 1.1 — Complete capability-ceiling coverage

1. Audit every mutation route against `observer-enforcement-inventory.md`; do not assume `require_admin` alone is sufficient.
2. Rule on relay participation: **decided — observers cannot mint relay tickets**. `POST /api/relay-ticket` now requires `submit_domain_command`; see `docs/DECISIONS.md` entry `P2P-2`.
3. Rule on bootstrap’s unauthenticated, token-gated, one-time path as a special client-class case: **decided — bootstrap remains intentionally unauthenticated and is excluded from capability checks**. It is the only unauthenticated write endpoint, is token-gated, and self-closes once any user exists. See `docs/DECISIONS.md` entry `P2P-3`.
4. Extend the existing observer test file to cover any newly ruled mutation path and the full §28 negative matrix. **DONE (2026-09-24).** `tests/mobile_observer_capability.rs` now asserts `CLIENT_CAPABILITY_DENIED` for every admin mutation route — user activation, password reset, manager/class/parent assignment, and batch profile import were added to the existing create/deactivate/mobile-access/profiles/policies/legal-holds coverage — plus relay-ticket (`P2P-2`), refresh preservation, and independent `TENANT_MISMATCH` isolation.

### Phase 1.2 — Add read endpoints for ObserverClient

**Status: DONE (2026-09-24).** File download and push-subscription
register/unregister routes exist in `crates/bins/api-server/src/routes/`
(`files.rs`, `push.rs`), registered in `routes/mod.rs`, backed by the
`push_subscriptions` table migration
(`migrations/{sqlite,postgres}/20260111000000_add_push_subscriptions`),
with positive E2E coverage in `tests/observer_read_routes.rs`. Capability
gates, the blob-store root, and the two disclosed follow-ups (FileAsset
tenant scoping → Phase 3.1; push delivery worker → Phase 3.2) are recorded
in `docs/DECISIONS.md` entry `P2P-4` and the route module docs.

What Phase 1.2 delivered:

- File download (`GET /api/files/:content_hash`)
- Push subscription register (`POST /api/push/subscriptions`)
- Push subscription unregister (`DELETE /api/push/subscriptions/:subscriptionId`)

All subject to `can_download_files` and `can_read_notifications`.

**Tests:** Negative E2E for mutations, positive E2E for reads.

---

## Phase 2 — PWA foundation (P2+P3) — `mobile-pwa/` greenfield

### Phase 2.1 — Project structure (parallel to `web-ui/`, but with PWA stack)

**Status: DONE (2026-09-24).** `mobile-pwa/` scaffolded with the structure
below. Build config (package.json, tsconfig app/node, vite.config.ts,
tailwind/postcss, .eslintrc.cjs, playwright.config.ts), `index.html` PWA
shell, `src/{api,components,hooks,lib,pages,routes,stores,types,utils}`,
`tests/setup.ts`, `.gitignore`, README.md. Vite dev server is `:5174`
(0.0.0.0) to run alongside `web-ui` on `:5173`; `VITE_API_BASE` defaults
to `http://127.0.0.1:3000`.

```text
mobile-pwa/
├── src/
│   ├── components/ (shared widgets)
│   ├── hooks/ (custom React hooks)
│   ├── pages/ (React Pages)
│   ├── routes/ (Wouter or React Router)
│   ├── api/ (Axios wrappers)
│   ├── utils/ (shared utilities)
│   └── lib/ (language wrappers)
├── public/
│   └── manifest.webmanifest (PWA metadata)
├── index.html (PWA shell)
├── vite.config.ts (Vite PWA config)
├── package.json / package-lock.json
├── tailwind.config.js
├── postcss.config.js
├── postinstall (generate service worker)
└── README.md (PWA-specific setup)
```

### Phase 2.2 — Build stack

**Status: DONE (2026-09-24).** React 18 · TypeScript 5.3 · Vite 5 · Tailwind
3.3 · Zustand 4 (mirrors `web-ui`) · TanStack Query + Axios · ESLint
(TS + react-hooks, `--max-warnings=0`) · Vitest 1/Playwright/axe-core.
PWA uses **plain Vite PWA manifest** (no PWAKit / vite-plugin-pwa): static
`public/manifest.webmanifest` + `public/icons/icon.svg`; the service worker
is generated by the `postinstall` script (`scripts/postinstall.mjs`) from
`scripts/sw.template.js`, stamping the app version into the cache name.
`npm install`, `type-check`, `lint`, `test`, `build` and a `vite preview`
serve-check all pass.

- Framework: React 18, TypeScript 5.3, Vite 5, Tailwind CSS 3.3
- State: Zustand (mirrors `web-ui/`)  
- Data fetching: TanStack React Query + Axios
- Linting: ESLint with TypeScript + React Hooks plugin
- Tests: Vitest + Playwright browser, axe-core (a11y)
- PWA: add `-disable` for PWAKit (use plain Vite PWA manifest)

### Phase 2.3 — PWA shell architecture

**Status: DONE (2026-09-24), with two explicitly deferred views.** Delivered:
`LoginScreen` (`/login`, always `client_type: "mobile_observer"`),
`ObserverLayout` (`/`, header + top nav to all six live views),
`DashboardView` (`/dashboard`, `dashboard.summary` + summary cards + recent
mission list + recent activity), `MissionsList` (`/missions`, `mission.list`),
`MissionDetail` (`/mission/:id`, `mission.detail {id}` + `timeline.list
{subject_id}`), `TasksList` (`/tasks`, `task.list`), `TaskDetail` (`/task/:id`,
`task.detail {id}`), `ApprovalView` (`/approvals`, `approval.list` — view-only;
no approve/reject path exists in the gateway), `NotificationsView`
(`/notifications`, `notification.list` — read-only; no acknowledge surface),
and `FileDetail` (`/files/:contentHash`, `downloadFile`). Deferred by
declared backend gaps:
- `EvidenceView` (`/evidence/:evidenceId`) — folded into the `Reports` page
  (`/reports`, `report.detail`), mirroring web-ui: evidence is surfaced as
  attachment references, not id-addressable records. A dedicated evidence view
  needs an `evidence.detail` query the backend does not offer.
- `FileList` (`/files`) — no FileAsset index exists (P2P-4 declares the
  Phase 3.1 follow-up); a hash-addressed file is still reachable at
  `/files/:contentHash`.

All views consume `QueryEnvelope` responses via TanStack Query
(`useObserverQuery(queryType, filters)`; web-ui-equivalent). Read-only is held
by absence: there are no mutation buttons and no acknowledge/approve helpers.

**ObserverClient surface (React components):**
- `LoginScreen` (`/login`) — Same as Flutter `http_login_screen.dart`? Mirror.
- `ObserverLayout` (`/`) — Shell with navigation (Dashboard/Flag/TaskAlt/Notifications/Approval/Settings)
- `DashboardView` (`/dashboard`) — Uses `/api/query?...` to load Dashboard projection
- `MissionsList` (`/missions`) — List of missions
- `MissionDetail` (`/mission/:id`)
- `TasksList` (`/tasks`)
- `TaskDetail` (`/task/:id`)
- `ApprovalView` (`/approvals`) — View-only; maybe `approvalRequirements` list
- `NotificationsView` (`/notifications`)
- `EvidenceView` (`/evidence/:evidenceId`)
- `FileList` (`/files`)
- `FileDetail` (`/files/:contentHash`)

All views consume `QueryEnvelope` responses via TanStack Query; may use Zustand store for local UI state (error, loading).

### Phase 2.4 — ObserverHttpGateway

**Status: DONE (2026-09-24).** Implemented in `mobile-pwa/src/api/onyx.ts`
(exported as `observerApi`). Deviations from the sketch below are
deliberate and documented in DECISIONS: `authenticate` always sends
`client_type: "mobile_observer"` (the gateway cannot self-escalate);
`query` takes a query_type/filters/options and encodes the envelope
base64url-no-pad itself; `refresh`/`logout` mirror web-ui's endpoint shapes;
`downloadFile` returns `{ contentHash, blob, fileName, size }`; a shared
read-only contract class type `ObserverHttpGateway` names the surface.
The client-side validation helpers (`isBase64Url`, `isHttpsEndpoint`,
`isContentHash`) mirror the Phase 1.2 backend rules.

```typescript
// src/api/onyx.ts
export const onyxApi = {
  authenticate: (u, p) => axios.post('/api/auth/login', { username, password }),
  refresh: (r) => axios.post('/api/auth/refresh', { refresh_token: r }),
  logout: () => axios.post('/api/auth/logout'),
  query: (q) => axios.get('/api/query', { params: q }),
  get: (url, config) => axios.get(url, config),
  downloadFile: (hash) => axios.get(`/api/files/${hash}`, { responseType: 'blob' }),
  registerPush: (sub) => axios.post('/api/push/subscriptions', sub),
  unregisterPush: (id) => axios.delete(`/api/push/subscriptions/${id}`),
};
```

### Phase 2.5 — Read endpoint alignment

**Status: DONE (2026-09-24).** Every contract ObserverClient method is mapped
to a live backend surface (details below). The PWA can read every projection
the backend offers to `mobile_observer`.

Contract surface → backend (query types per `query_handler.rs`):

| Contract method                                    | Backend                                                               | Status |
| -------------------------------------------------- | --------------------------------------------------------------------- | ------ |
| `authenticate` / `refreshSession` / `logout`        | `POST /api/auth/login` (`client_type: mobile_observer`) / `refresh` / `logout` | live |
| `getDashboard`                                     | `dashboard.summary`                                                   | live |
| `listMissions` / `getMission`                       | `mission.list` / `mission.detail {id}`                                | live |
| `listTasks` / `getTask`                             | `task.list` / `task.detail {id}`                                      | live |
| `listApprovalRequirements` / `getApprovalRequirement` | `approval.list` / `approval.list {id}` (no `approval.detail`)         | live |
| `listNotifications`                                | `notification.list`                                                   | live |
| `getEvidence`                                      | evidence refs inside `report.detail` (label + `file_name`)            | partial → Reports page |
| `getAuditView`                                     | `timeline.list {subject_id}` (state-transition timeline)              | partial → MissionDetail |
| `getHierarchyView`                                 | `GET /api/users/hierarchy` (authenticated, same-org, not admin-gated) | live; no page yet |
| `getFileMetadata`                                  | none (hash download returns bytes only)                               | Phase 3.1 |
| `downloadFile`                                     | `GET /api/files/:content_hash` (`can_download_files`)                 | live |
| `registerPushSubscription` / `unregisterPushSubscription` | `POST/DELETE /api/push/subscriptions...` (`can_read_notifications`)   | live (UI = Phase 3.2) |

Notable maps and gaps: `getEvidence` is not id-addressable — evidence exists
only as attachment references on report projections, so the PWA surfaces them
on `/reports` as view-only references (web-ui parity). `getAuditView` is backed
by the timeline projection; there is no full audit-log query. `getFileMetadata`
has no server-side source (a hash resolves to bytes, nothing more) — declared
Phase 3.1 with the FileAsset index. Profile reads (`/api/profiles`,
`/api/profiles/:owner_id`) are available to the observer but not yet wired into
a page. Unknown query types return empty results, never errors (contract §read
inventory).

Compare with `mobile-core`'s read operations:
- `mobile_core_list_aggregates` → Dashboard, Missions, Tasks, Notifications, Approvals? (Approval may need custom query)
- `mobile_core_execute_query` → custom query for missing projection (maybe used for approval requirements?)
- `mobile_core_get_sync_status` → sync status widget
- `mobile_core_list_conflicts` → conflict indicator

Ensure ObserverClient can read all projections the backend offers with `mobile_observer` capability.

### Phase 2.6 — Observer PWA tests

**Status: DONE.** The observer test suite is complete and green on every gate
(`npm run type-check`, `npm run lint`, `npm run build`, `npm run test`,
`npm run test:a11y`, `npm run test:browser` — 27 Vitest + 5 Playwright tests).

- Unit tests: `tests/unit/validation.test.ts`, `envelope.test.ts`,
  `errorHandler.test.ts` (15 tests)
- Component tests (`tests/component/`): Login posts `client_type:
  mobile_observer` and only authenticates on success, protected shell
  redirects unauthenticated visits and signs out, and the Missions /
  Notifications / Approvals views assert the decision-free surface by
  absence of Acknowledge/Approve/Reject buttons. The onyx gateway is mocked
  per-file with `vi.mock` + `vi.importActual` (only `query` is replaced).
- a11y tests (`tests/accessibility/`): axe-core checks for StatusBadge,
  Freshness, Login, and a populated Notifications list; asserts
  `results.violations` toHaveLength(0) (the `toHaveNoViolations` matcher in
  vitest-axe 0.1.0 ships as an empty `extend-expect` build, see P2P-7).
- E2E (`tests/browser/observer.spec.ts`, fixtures in
  `tests/browser/fixtures/onyx.ts`): routes `/api/*` in-browser, keyed on the
  base64url `envelope` query_type; covers observer login (asserts the
  `mobile_observer` client class in the POST body), seeded-session projection
  reads, mutation denial across notifications and approvals, file download
  with `Authorization: Bearer` verification, and the not-found fallback.
  Playwright runs against the system Chrome via `channel: 'chrome'` because
  `npx playwright install chromium` fails to download browsers in this
  environment (see P2P-7). Sessions are seeded with the observer keys
  (`onyx_observer_access_token` / `onyx_observer_refresh_token` /
  `onyx_observer_user`), distinct from web-ui's keys.

---

## Phase 3 — PWA files + push (P4+P5)

### Phase 3.1 — File download

**Status: DONE.** `downloadFile` landed in `ObserverHttpGateway` with Phase
2.4 (`src/api/onyx.ts`) and `GET /api/files/:content_hash` is exercised by
the FileDetail view (`/files/:contentHash`), which saves the blob with the
hash as its filename. Phase 2.6's E2E (`file download carries the bearer
token and reports bytes`) proves the full path, asserting the
`Authorization: Bearer` header.

The plan's *FileList → click download* surface stays deferred: api-server
has no FileAsset listing route (DECISIONS P2P-6 §4), so there is no
first-party way to enumerate a user's files to build a `/files` index. That
listing is the pre-requisite for a real FileList view.

- Add `downloadFile` to ObserverHttpGateway (see Phase 2.4) — **delivered**
- Frontend: FileList → click download triggers `downloadFile` → blob → save
  — delivered as `/files/:contentHash` (FileDetail); FileList index defers on
  a backend FileAsset listing route

### Phase 3.2 — Web Push

**Status: DONE.** The PWA side of Web Push is delivered (DECISIONS P2P-8);
the server-side **delivery worker** landed here too (DECISIONS P2P-10).

- Register Service Worker (SW) with Push notification permission — SW
  registered app-wide (`src/lib/pwa.ts`); the Notifications view's
  `PushNotificationsCard` requests permission and creates the browser
  `PushSubscription` (`src/push/push.ts`).
- SW receives push events and displays native notification — `public/sw.js`
  `push` handler shows `{title, message, url}` payloads; E2E simulates a
  frame dispatched into the live SW.
- Notification click opens ObserverClient to target view — `notificationclick`
  navigates/focuses the controlling window client to `data.url`.
- Subscribe/unsubscribe via API calls from frontend — `subscribeToPush` →
  `POST /api/push/subscriptions` (returns stored id), `unsubscribeFromPush`
  → `DELETE /api/push/subscriptions/:subscription_id` plus a local
  unsubscribe. The enabled state persists under `onyx_observer_push`.

**Push delivery worker — DELIVERED (2026-09-24).** `crates/bins/worker`
now fans unacknowledged `notification` aggregates out to registered push
endpoints (`crates/bins/worker/src/push_delivery.rs` + `webpush.rs`,
migration `20260112000000_add_push_deliveries`):

- Every poll tick loads `push_subscriptions`, targets the notification
  aggregates whose `state.recipient_id` matches the subscription `user_id`
  and `status = 'unacknowledged'`, and skips anything already recorded in the
  `push_deliveries` ledger (idempotent per `(subscription_id,
  notification_id)`; a row is written *only* on success).
- Payloads are VAPID-signed (ES256 JWT per RFC 8292) and encrypted per RFC
  8291 (`aes128gcm`, ECDH P-256 + HKDF + AES-128-GCM) with `ring`; the
  `Authorization`, `TTL`, `Content-Encoding`, `Urgency` headers are built
  per RFC 8292/8030. `404/410` responses prune the stale subscription.
- Config comes from the environment: `ONYX_VAPID_PRIVATE_KEY_PKCS8_BASE64`
  (the PKCS#8 DER base64url `npx web-push generate-vapid-keys` prints, whose
  matching public key the PWA verifies via `VITE_VAPID_PUBLIC_KEY`),
  `ONYX_VAPID_SUBJECT`, and `ONYX_PUSH_*` tuning knobs.
- Tests: 8 unit tests cover the VAPID JWT, RFC 5869 HKDF, and an
  encrypt→decrypt round-trip proved against the subscription keys; 2 live
  Postgres-gated tests cover ledger dedup and stale-subscription pruning
  (they run when `DATABASE_URL` is a Postgres URL, mirroring
  `staff_loan_scheduler`).

Remaining gate for *device-level* acceptance is a real push service round
trip (Firebase/Cloud Messaging) on a physical device — delivery is
implemented and unit/integration-tested, not yet proven against FCM.

### Phase 3.3 — Tests

**Status: DONE.**

- Push integration: Playwright EMmitter API; simulate push events via SW
  test — `tests/browser/push.spec.ts` dispatches a real `PushEvent` into the
  controlling service worker and asserts the shown notification, then
  dispatches `NotificationEvent('notificationclick')` and asserts navigation
  to the payload target. Opt-in/opt-out is covered with a deterministic
  `PushManager` stub and asserted POST/DELETE bodies on
  `/api/push/subscriptions`.
- File download: E2E download test (verify file saved with correct name/content)
  — the Phase 2.6 browser test asserts the authenticated download and the
  reported byte count; the FileDetail page saves the blob under the
  content-hash filename.

---

## Phase 4 — Android completion (A6) (Hardware gated)

### Phase 4.1 — P2P transport (Kotlin→JNI)

**Locked direction:** `docs/DECISIONS.md` entry `P2P-1`.

**Work:**

- Add Kotlin `WifiP2pManager`/`BluetoothLeScanner` platform drivers.
- Add corresponding Rust JNI transport entry points for framing/encryption/handshake behavior.
- Replace the placeholder C-ABI symbol stubs rather than extending them as if they performed transport.
- Run P2P coverage in instrumented Android tests on real hardware.

**Code subset — done in sandbox (see additionally `DECISIONS.md` `P2P-11`):**

- **Rust framing/encryption/handshake** implemented in `crates/mobile-android-jni/src/p2p.rs` and exposed via JNI class `com.onyx.p2p.P2pCodec` (`nativeSessionStart/Accept/ClientMessage/ServerMessage/Complete/Encode/Decode/Close`). Design: ephemeral P-256 ECDH, HKDF-SHA-256 key schedule bound to `PROLOGUE ‖ client_pub ‖ server_pub`, AES-256-GCM, per-direction keys, HMAC-derived nonces from monotonic counters, `u32-BE length ‖ version` 5-byte framing, strict desync-on-failure semantics. Verified on host: 6/6 unit tests pass, `clippy -D warnings` clean, `cargo fmt` clean.
- **Kotlin drivers** in `mobile-android/app/src/main/kotlin/com/onyx/p2p/{WifiDirectDriver,BleDriver,P2pChannel,P2pStream,P2pCodec}.kt`; connectivity manifest permissions + optional `uses-feature` added.
- **Placeholder C-ABI stubs deleted** (not extended, per P2P-1): `sync-transport-mobile`'s `android_wifi_direct`/`android_ble` modules and `mobile-core`'s corresponding re-export modules are gone; their crate doc comments record the removal.
- **Still deferred (hardware gate):** on-device P2P tests under 4.6; drivers compile only in CI (no local Android SDK).

### Phase 4.2 — Background work verification

- Verify WorkManager scheduling (`scheduleBackgroundSync`)
- Test background sync execution under Doze mode, airplane mode
- Ensure sync agent runs and persists state correctly
- Verify `WorkManagerService` onReceive calls correct mobile-core functions

**Code subset — done in sandbox (see additionally `DECISIONS.md` `P2P-11`):**

- Scheduling contract is now asserted by instrumented test `BackgroundSyncInstrumentedTest` (`androidTest`): `UNIQUE_WORK_NAME` idempotency under `ExistingPeriodicWorkPolicy.KEEP`, periodic type, and `NetworkType.CONNECTED` constraint, via `WorkManagerTestInitHelper`. Runs under `connectedAndroidTest` (hardware gate), mirroring `MobileCoreRoundTripTest`'s disclosure comment.
- Runtime behavior under Doze/airplane mode and the service → mobile-core function wiring remain hardware/lab gates; nothing here changes `WorkManagerService`'s existing forward path.

### Phase 4.3 — Offline behavior

- Ensure local SQLite state persists when offline
- Verify that commands queued during offline still sync when network returns
- Conflict resolution local testing

### Phase 4.4 — Accessibility audit

- Run TalkBack/Explore-by-Touch tests
- Verify semantic labels, focus order, color contrast
- Automated a11y tests via `axe-core` (or Android equivalent)

### Phase 4.5 — Release artifact

- Sign the APK (debug signing for now)
- Add ProGuard/R8 rules
- Create release build variant
- Upload to internal artifact repository

**Code subset — done in sandbox:**

- `app/proguard-rules.pro` (JNI name-mangling keeps, `WorkManagerService` worker keep) + `release` buildType with R8 minify + resource shrink + debug-keystore signing, and `assembleRelease` added to the `mobile-android-kotlin` CI job so R8 is exercised on every run. Artifact upload adds the R8 release APK alongside debug.
- **Still deferred:** dedicated release signing/production-evidence pipeline (Phase 6) and on-device survival check of the R8 APK (lab gate).

### Phase 4.6 — CI verification

- Run `mobile-android-kotlin` job on real device/emulator
- Instrumented tests: `connectedAndroidTest`
- Verify JNI round-trip test passes on device
- Run load tests if possible

---

## Phase 5 — PWA acceptance (P6) + deployment (PWA hosting decision pending)

### Phase 5.1 — Home Screen installation

- Add to `mobile-pwa/`:
  - `manifest.webmanifest` with `theme-color`, `background-color`
  - `sw.js` (Service Worker)
  - `manifest.json` (PWABuilder may generate)
  - `icons/` (various sizes)
  - `screen.html` (launch screen)

- Vite PWA plugin (or manual generation) to produce `sw.js` and pre-cache the shell.

**Status: 5.1 DELIVERED** — `manifest.webmanifest`, `icons/icon.svg` and the
dev SW were already in place from Phase 2.2. This pass closes the rest:
`public/screen.html` (standalone launch screen, links onward to `/`), the iOS
standalone meta tags in `index.html` (`mobile-web-app-capable`,
`apple-mobile-web-app-*`), and **build-time shell pre-caching**:
`vite.config.ts` now emits a build manifest (`build.manifest:true`) and
`scripts/render-sw.mjs` (chained after `vite build`) stamps the hashed asset
list + `/` + icon + manifest into `dist/sw.js` via the `__PRECACHE_URLS__`
template slot, so the worker is install-time offline-first. Dev keeps
`public/sw.js` (from `scripts/postinstall.mjs`) with an empty precache list.

### Phase 5.2 — Standalone behavior

- Test PWA launch from Home Screen (iPhone, not Safari WebView)
- Verify offline shell works (SW cache serves `index.html`)
- Verify push notifications arrive (use Firebase/Cloud messaging test)

**Status: 5.2 DELIVERED (automated subset)** — the app now surfaces offline
state (`src/hooks/useOnline.ts` + `src/components/OfflineBanner.tsx`, mounted
in `ObserverLayout`), and the SW serves the shell offline: navigations are
network-first with fallback to the precached `/`; `/api` stays network-first
falling back to the last cached snapshot; cached responses are sanitized
(strip `Vary`/CORS headers) so Cache Storage matching cannot be broken by a
host that stamps `Vary: Origin` (vite preview does). `playwright.offline
.config.ts` + `tests/offline/offline.spec.ts` (`npm run test:browser:offline`,
builds with `VITE_API_BASE=http://localhost:5175`) prove: after install the
shell renders offline, the offline banner shows, and the seeded last snapshot
(`mission.list` envelope) is served from the SW cache. **Still hardware-gated
on real iPhone:** Home Screen launch behavior, `notificationclick` focus
semantics on iOS, and the final device-level proof of end-to-end push
delivery: the delivery worker now exists (Phase 3.2, DECISIONS P2P-10) but
pushing through a real push service (FCM/etc.) to a physical device remains
cloud/hardware-gated.

### Phase 5.3 — Security & CI

- Run security scans (npm audit, OSV, etc.)
- Run lint/type/unit tests
- Run Playwright E2E including PWA (headful or headless)
- Accessibility tests (`axe-core`)

**Status: 5.3 DELIVERED** — `npm audit` went from 6 findings to **0
vulnerabilities**: dev toolchain upgraded `vite` 5→7, `vitest` 1→4 (plus
`@vitejs/plugin-react` and explicit `@types/node` for the dropped ambient
Buffer globals), and `react-router-dom` 6.30→7.18 (v6 has no patched release
for GHSA-wrjc / GHSA-337j; the client renders client-side only, neither CVE
is exploitable here, but 7.x closes both). All gates green:
`type-check`, `lint`, `build` (which also renders `dist/sw.js`), `npm run
test` (38 Vitest), `npm run test:a11y` (axe-core), `npm run test:browser`
(7 Playwright dev-suite), `npm run test:browser:offline` (1 Playwright
prod-suite offline test).

### Phase 5.4 — Hosting decision & deployment

**Option 1: New host + Helm**
- Create Helm chart in `deploy/onyx-pwa` (values, ingress, service)
- Deploy to EKS with RDS (shared? separate?)
- Update `api-server` CORS allow-list for the PWA origin
- DNS alias (e.g., `observer.onyx.example.com`)

**Option 2: Shared host**
- Mount PWA static files under `api-server` ingress path (e.g., `/pwa/`)
- Add CORS allow-list entry for web-ui origin (if same origin)

**Decision:** defer to Phase 5.4.1; whichever option is chosen, update `api-server` CORS config and document in `docs/mobile-migration/observer-hosting-choice.md`

---

## Phase 6 — Production evidence

### Phase 6.1 — Lockfiles & signed artifacts

- Commit `Cargo.lock` (updated after any Rust changes)
- Commit `web-ui/package-lock.json`
- Commit `mobile-pwa/package-lock.json`
- GPG sign all binary releases (api-server, worker, sync-agent, migration-tool, desktop-shell, admin-shell)

### Phase 6.2 — Device-lab transcripts

- Run Android instrumented tests on real device (record logs)
- Record iPhone PWA acceptance test logs
- Archive in `artifacts/`

### Phase 6.3 — Negative test evidence

- CI report for capability-negation-e2e (§28)
- Test logs for each environment showing mutation attempts result in 403.

---

## Coordination

All phases depend on the prior ones:
- **Phase 1** must finish before **Phase 2–3** can start (backend must enforce observer capabilities).
- **Phase 4** can run in parallel with **Phase 2–3**, but requires hardware.
- **Phase 5** depends on **Phase 3** (PWA files/push) and **Phase 4** (device access).

Key dependencies:
- Observer capability enforcement in backend (Phase 1) for both Android and PWA security boundaries.
- Read endpoint verification in backend (Phase 1) for PWA compatibility.
- P2P transport implementation (Phase 4) for Android acceptance.
- PWA hosting decision (Phase 5) for production deployment.

---

## Completion Criteria

A phase is complete when:
1. All implementation tasks in that phase are delivered.
2. The designated CI gates (e.g., `mobile-android-kotlin`, `load-smoke`, `native-ui-evidence`) pass on the appropriate hardware/environment.
3. All acceptance tests pass (negative capability E2E, read projection correctness, accessibility, etc.).
4. Documentation and contract updates are committed.

The migration is considered **DONE** when:
- Android Operational Client passes all acceptance gates.
- iOS Observer Client passes all acceptance gates.
- Observer capability enforcement is production-ready.
- Flutter is retired (greenfield no longer in repo).
- All new implementation is version-controlled, documented, and CI-verified.

---

## References

- `docs/mobile-migration/observer-capability-matrix.md`
- `docs/mobile-migration/observer-enforcement-inventory.md`
- `docs/mobile-migration/android-jni-contract.md`
- `docs/mobile-migration/pwa-observer-contract.md`
- `docs/mobile-migration/pwa-capability-matrix.md`
- `docs/mobile-migration/flutter-retirement-record.md`
- `docs/DECISIONS.md` entries `FLT-1` and `P2P-1`

---

**Plan Status: PHASE 0 IN PROGRESS**

Phase 0 contracts are being frozen from verified source. Later-phase implementation details remain plans, not completed work.
