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

**Status: SCAFFOLDED — views pending.** So far: `LoginScreen` (functional,
`client_type: "mobile_observer"`), `ObserverLayout` (header + top nav with
disabled placeholders), `DashboardView` (WIP using `GetDashboard`/
`ListMissions`/`ListNotifications`/`ListPendingApprovals` queries), 404
page, and the route table in `src/routes/index.tsx`. The full view list
below is the remaining work of this phase.

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

Compare with `mobile-core`’s read operations:
- `mobile_core_list_aggregates` → Dashboard, Missions, Tasks, Notifications, Approvals? (Approval may need custom query)
- `mobile_core_execute_query` → custom query for missing projection (maybe used for approval requirements?)
- `mobile_core_get_sync_status` → sync status widget
- `mobile_core_list_conflicts` → conflict indicator

Ensure ObserverClient can read all projections the backend offers with `mobile_observer` capability.

### Phase 2.6 — Observer PWA tests

- Unit tests: Vitest component tests, hook tests
- a11y tests: Playwright + axe-core; ensure focus order, screenreader support
- E2E: Playwright browser tests cover login, read projections, mutation denials, file download

---

## Phase 3 — PWA files + push (P4+P5)

### Phase 3.1 — File download

- Add `downloadFile` to ObserverHttpGateway (see Phase 2.4)
- Frontend: FileList → click download triggers `downloadFile` → blob → save

### Phase 3.2 — Web Push

- Register Service Worker (SW) with Push notification permission
- SW receives push events and displays native notification
- Notification click opens ObserverClient to target view (e.g., `/notifications`)
- Subscribe/unsubscribe via API calls from frontend.

### Phase 3.3 — Tests

- Push integration: Playwright EMmitter API; simulate push events via SW test.
- File download: E2E download test (verify file saved with correct name/content)

---

## Phase 4 — Android completion (A6) (Hardware gated)

### Phase 4.1 — P2P transport (Kotlin→JNI)

**Locked direction:** `docs/DECISIONS.md` entry `P2P-1`.

**Work:**

- Add Kotlin `WifiP2pManager`/`BluetoothLeScanner` platform drivers.
- Add corresponding Rust JNI transport entry points for framing/encryption/handshake behavior.
- Replace the placeholder C-ABI symbol stubs rather than extending them as if they performed transport.
- Run P2P coverage in instrumented Android tests on real hardware.

### Phase 4.2 — Background work verification

- Verify WorkManager scheduling (`scheduleBackgroundSync`)
- Test background sync execution under Doze mode, airplane mode
- Ensure sync agent runs and persists state correctly
- Verify `WorkManagerService` onReceive calls correct mobile-core functions

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

### Phase 5.2 — Standalone behavior

- Test PWA launch from Home Screen (iPhone, not Safari WebView)
- Verify offline shell works (SW cache serves `index.html`)
- Verify push notifications arrive (use Firebase/Cloud messaging test)

### Phase 5.3 — Security & CI

- Run security scans (npm audit, OSV, etc.)
- Run lint/type/unit tests
- Run Playwright E2E including PWA (headful or headless)
- Accessibility tests (`axe-core`)

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
