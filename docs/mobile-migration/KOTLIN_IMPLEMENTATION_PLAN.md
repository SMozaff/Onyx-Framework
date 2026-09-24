# Kotlin Android Implementation Plan

**Document:** `docs/mobile-migration/KOTLIN_IMPLEMENTATION_PLAN.md`  
**Date:** 2026-09-24  
**Basis:** ONYX_MOB-01 §25 Android Work Packages (A0–A6), ONYX_Android_Kotlin_iOS_PWA_Technical_Blueprint v1.1, `docs/mobile-migration/parity-matrix.md`, and verified against actual source in `mobile-android/`, `crates/mobile-core/`, `crates/mobile-android-jni/`.

**Context:** Flutter was retired via DECISIONS.md entry FLT-1 (2026-09-24). The Flutter retirement gate was NOT met (JNI never run on real device, P2P transport stubbed with `ConnectionLost` placeholders, background/offline behavior unverified, no rollback artifact). The owner explicitly overrode the gate and proceeded with deletion. Kotlin must still close the outstanding acceptance gate (Layer 6) before any production mobile release.

---

## Current State Summary (verified against actual source, not docs)

| Layer | Status | Evidence |
|---|---|---|
| **0 — Freeze/inventory** | ✅ Done (Flutter deleted) | `mobile/` removed; parity-matrix.md retained |
| **1 — Kotlin project skeleton** | ✅ Mostly complete | `mobile-android/` has real Compose project, build config, CI |
| **2 — JNI boundary** | ⚠️ Declared, mostly implemented | `mobile-android-jni/` exposes 14 `MobileCoreBridge` entry points (12 real — incl. the real `nativeSubscribeEvents`/`nativeUnsubscribe` pair, added 2026-09-24; `nativeExecuteQuery` real but unwired; `nativeSecureStorage` stub removed) **plus the Phase 4.1 P2P codec** — 8 `com.onyx.p2p.P2pCodec` sessions functions, all implemented, 6/6 host tests green. See `android-jni-contract.md`. |
| **3 — Startup/auth** | ⚠️ Partially implemented | `OnyxSessionViewModel.kt`, `AuthApi.kt`, `SecureTokenStore.kt` exist; `ffi_secure_storage` not yet bound |
| **4 — Core read screens** | ⚠️ Partially implemented | Dashboard, Missions, Tasks, MissionDetail, Notifications screens exist; `OnyxController` refresh fan-out works |
| **5 — Operational screens** | ⚠️ Partially implemented | Approvals, Files, Settings, ConflictDialog, SyncStatusIndicator, WorkManager exist; background sync schedule asserted by instrumented test (`BackgroundSyncInstrumentedTest`, hardware-gated) but execution unverified |
| **6 — Real-device acceptance** | ⚠️ Code subset done; device gate open | P2P codec + Kotlin drivers written (see MIGRATION_PLAN 4.1 status), placeholder C-ABI stubs deleted; no device testing yet, background/offline execution unverified |

---

## Layer 0 — Freeze/inventory of what Flutter did

**What already exists:** `docs/mobile-migration/parity-matrix.md` is the frozen M0 parity baseline — every line reflects what `mobile/lib/` did before deletion. This is the acceptance criteria the Kotlin rewrite must match.

**What's missing:** The parity matrix was derived from reading the now-deleted `mobile/lib/` source. The matrix itself is preserved as historical record.

**Next concrete task:** None — this layer is complete. Use `parity-matrix.md` as the reference for feature-by-feature verification in later layers.

---

## Layer 1 — Kotlin project skeleton

**What already exists (with path evidence):**

- `mobile-android/build.gradle.kts` — root build script with `com.android.application` 8.13.2, `org.jetbrains.kotlin.android` 2.3.21, `org.jetbrains.kotlin.plugin.compose` 2.3.21
- `mobile-android/app/build.gradle.kts` — `compileSdk 36`, `minSdk 29`, `targetSdk 36`, Compose enabled, `jniLibs.srcDirs("src/main/jniLibs")`, dependencies: compose-BOM 2026.03.00, material-icons-extended, lifecycle-runtime-ktx 2.8.7, lifecycle-viewmodel-compose 2.8.7, work-runtime-ktx 2.9.1, okhttp 4.12.0, kotlinx-coroutines-android 1.9.0, org.json:json 20250517 (test)
- `mobile-android/app/src/main/AndroidManifest.xml`
- `mobile-android/app/src/main/res/values/strings.xml`, `themes.xml`
- `mobile-android/app/src/main/kotlin/com/onyx/ui/AppShell.kt` — bottom-nav shell with Dashboard/Flag/TaskAlt/Notifications/Approval/Settings/WarningAmber icons
- `mobile-android/app/src/main/kotlin/com/onyx/MainActivity.kt` — real startup entry point rendering Loading/NeedsLogin/StartupError/Ready states; `LaunchedEffect(current.handle)` triggers `scheduleBackgroundSync`
- `mobile-android/app/src/main/kotlin/com/onyx/OnyxApplication.kt`
- `mobile-android/.gitignore`, `settings.gradle.kts`, `gradle.properties`, `gradlew`, `gradle/wrapper/`
- CI: `mobile-android-kotlin` job in `.github/workflows/ci.yml` (cargo-ndk, ndk r28b, Gradle assembleDebug, uploads APK artifact; Phase 4.5 adds `assembleRelease` with R8 + debug signing)
- `mobile-android/tool/build_rust_jni.sh` — builds native libraries via cargo-ndk

**What's missing:**
- Navigation component dependency (AppShell uses manual state switching; no `NavHost` Compose Navigation library yet) — this is a design choice, not a gap
- `src/main/jniLibs/` directory (where `.so` files go) — doesn't exist yet; will be populated by `build_rust_jni.sh`

**Next concrete task:**
1. Verify `build_rust_jni.sh` produces `.so` files for all three ABIs (`arm64-v8a`, `armeabi-v7a`, `x86_64`) and places them in `src/main/jniLibs/`
2. Add the Compose Navigation dependency if `NavHost`-based navigation is desired (currently manual)

---

## Layer 2 — JNI boundary (`mobile-android-jni`)

**What already exists (with path evidence):**

`crates/mobile-android-jni/src/lib.rs` exposes `#[no_mangle] pub extern "system"` JNI entry points under `Java_com_onyx_bridge_MobileCoreBridge_*`. The `MobileCoreBridge` surface is 14 functions: 12 are real wrappers (including `nativeExecuteQuery`, still unwired at the Kotlin call-site level), and the event pair `nativeSubscribeEvents` / `nativeUnsubscribe` are now **real** as of 2026-09-24 (context-carrying C ABI + per-event JVM attach — `DECISIONS M11-D12`; design in `android-jni-contract.md`). The former `nativeSecureStorage` scaffold is **removed** (Keystore in `SecureTokenStore.kt` is the sanctioned secret store). See `docs/mobile-migration/android-jni-contract.md` for the exact frozen contract and open items.

`MobileCoreBridge.kt` (`mobile-android/app/.../bridge/MobileCoreBridge.kt`) declares the same set; the subscription pair is declared with the real `EventCallback` fun-interface parameter. `EventCallback.kt` (new) is the Kotlin-side callback interface the native forwarder invokes.

| JNI function | Kotlin declaration | mobile-core C function wrapped | Purpose |
|---|---|---|---|
| `nativeNew` | `MobileCoreBridge.nativeNew(dbPath, configJson): Long` | `mobile_core_new` | Handle lifecycle |
| `nativeFree` | `MobileCoreBridge.nativeFree(handle: Long)` | `mobile_core_free` | Handle lifecycle |
| `nativeSetHierarchy` | `MobileCoreBridge.nativeSetHierarchy(handle, hierarchyJson): Int` | `mobile_core_set_hierarchy` | A3 auth/authority |
| `nativeExecuteCommand` | `MobileCoreBridge.nativeExecuteCommand(handle, commandJson): String?` | `mobile_core_execute_command` | A4/A5 commands |
| `nativeListAggregates` | `MobileCoreBridge.nativeListAggregates(handle, aggregateType): String?` | `mobile_core_list_aggregates` | A4 lists |
| `nativeGetSyncStatus` | `MobileCoreBridge.nativeGetSyncStatus(handle): String?` | `mobile_core_get_sync_status` | A4 Dashboard |
| `nativeListConflicts` | `MobileCoreBridge.nativeListConflicts(handle): String?` | `mobile_core_list_conflicts` | A4 Dashboard |
| `nativeUploadFile` | `MobileCoreBridge.nativeUploadFile(handle, path, orgId, userId, deviceId): String?` | `mobile_core_upload_file` | A5 Files |
| `nativeDownloadFile` | `MobileCoreBridge.nativeDownloadFile(handle, contentHash, destPath): Long` | `mobile_core_download_file` | A5 Files |
| `nativeTriggerSync` | `MobileCoreBridge.nativeTriggerSync(handle): Int` | `mobile_core_trigger_sync` | A5 sync |
| `nativeResolveConflict` | `MobileCoreBridge.nativeResolveConflict(handle, conflictJson, resolution): Int` | `mobile_core_resolve_conflict` | A5 conflicts |
| `nativeExecuteQuery` | `MobileCoreBridge.nativeExecuteQuery(handle, queryJson): String?` | `mobile_core_execute_query` | A4/A5 reads — wrapped, unwired |
| `nativeSubscribeEvents` | `MobileCoreBridge.nativeSubscribeEvents(handle, filterJson, callback: EventCallback): Long` | `mobile_core_subscribe_events` | Real-time push — implemented, consumed by `OnyxController` |
| `nativeUnsubscribe` | `MobileCoreBridge.nativeUnsubscribe(subscription: Long)` | `mobile_core_unsubscribe` | Real-time push — implemented |

`MobileCoreBridge.kt` (`mobile-android/app/.../bridge/MobileCoreBridge.kt`) declares all of the above as `external fun` matching the JNI name-mangling (JNI names depend only on method names, not signatures, so the subscription pair's richer Kotlin signature adds no ABI risk).

**Gaps in `mobile-android-jni`:**
The module doc’s older “remaining ~14 functions” note predates the current scaffold and is no longer accurate as a count. The actual remaining JNI gaps are:
- `mobile_core_execute_query` is wrapped but unwired — add a real Kotlin query path (`QueryEnvelope` builder + detail-screen call site), JSON schema, and tests.
- Event subscription is implemented end-to-end but only consumed by `OnyxController`; a direct Mission/Task-detail live-refresh call site beyond the org-wide filter is future polish. Real-device delivery through a live `EventCallback` is a lab gate (proven at the C-ABI boundary via `ffi_integration.rs`).
- `mobile_core_secure_storage_*` does not exist: `mobile-core/src/ffi_secure_storage.rs` explicitly remains unimplemented. Resolved 2026-09-24 (M11-D12.4): `SecureTokenStore.kt`'s Android Keystore path is the sanctioned secret store; the `nativeSecureStorage` stub is deleted and no JNI function will exist until a real `mobile_core_*` export does.
- `mobile_core_ios_background_sync` is iOS-only and not needed for Android.
- `mobile_core_android_do_work` is already called directly from Kotlin’s `WorkManagerService.kt`, not through this JNI crate.
- `mobile_core_free_string` is an internal ownership helper, not a separate JNI entry point.

**Missing `mobile-core` FFI functions not yet exposed via JNI:**
- `mobile_core_execute_query` (`ffi_queries.rs`) — JNI wrapper exists (`nativeExecuteQuery`); the Kotlin query path is unwired.
- `mobile_core_subscribe_events` / `mobile_core_unsubscribe` (`ffi_events.rs`) — **wrapped** (2026-09-24).
- `mobile_core_secure_storage_*` — deliberately not wrapped; no such export exists (see above).

**Next concrete task:**
1. Wire `nativeExecuteQuery` to a real Kotlin query path with its JSON schema and tests.
2. ~~Decide the JNI event-callback design, then implement usable `nativeSubscribeEvents` / `nativeUnsubscribe` with a subscription-handle registry.~~ Done 2026-09-24 (M11-D12): context-carrying C ABI, static trampoline, opaque `Long` handle, no Kotlin registry.
3. ~~Decide secure-storage direction.~~ Done 2026-09-24: keep Keystore in `SecureTokenStore.kt`; remove `nativeSecureStorage`.
4. Add adapter/instrumented coverage for each newly completed JNI path on real Android hardware.

---

## Layer 3 — Startup/auth

**What already exists (with path evidence):**

- `mobile-android/app/src/main/kotlin/com/onyx/session/OnyxSessionViewModel.kt` (268 lines) — real startup state machine: `Loading` → `NeedsLogin` → `Ready` (or `StartupError`). No manual identity entry anywhere. Recovery from `StartupError` is `retry()` or `signOutAndRetry()` only. Handles saved-session restoration via `SecureTokenStore`.
- `mobile-android/app/src/main/kotlin/com/onyx/session/SessionPreferences.kt` — stores server address preference
- `mobile-android/app/src/main/kotlin/com/onyx/security/SecureTokenStore.kt` — Android Keystore-backed token storage
- `mobile-android/app/src/main/kotlin/com/onyx/net/AuthApi.kt` (170 lines) — real login/refresh against `api-server` routes using OkHttp. `LoginResult`, `RefreshResult` data classes. `MobileAccessRestrictedException` for mobile-class access denial.
- `mobile-android/app/src/main/kotlin/com/onyx/session/OnyxUiState.kt` (inline in OnyxSessionViewModel) — `Loading`, `NeedsLogin`, `Ready(handle, orgId, userId)`, `StartupError`
- `mobile-android/app/src/main/kotlin/com/onyx/controller/OnyxController.kt` (346 lines) — takes `handle`, `organizationId`, `userId`, `Context` in `Factory`; owns the shared-refresh fan-out of 6 parallel FFI calls. `OnyxController.Factory` is used by `MainActivity`.
- `mobile-android/app/src/main/kotlin/com/onyx/bridge/MobileCoreBridge.kt` — `nativeSetHierarchy` declared and wired

**Gaps:**
- `SecureTokenStore` uses Android Keystore directly — confirmed as the sanctioned path 2026-09-24 (M11-D12.4); `ffi_secure_storage.rs` exports no functions, so no Rust binding is needed.
- `mobile_core_execute_query` JNI wrapper exists but the Kotlin query path is unwired (needed for query-type auth checks and detail-screen reads).
- No instrumented test verifying the full login flow through JNI → mobile-core → server on real hardware

**Next concrete task:**
1. ~~Verify `SecureTokenStore` correctly calls `mobile_core` secure storage~~ Resolved 2026-09-24: Keystore is the intended path; no Rust secure-storage export exists.
2. Wire the Kotlin query path over the existing `nativeExecuteQuery` JNI wrapper.
3. Write instrumented test: full login → hierarchy load → `OnyxController` initialization on real device
4. ~~Add `mobile_core_subscribe_events` JNI wrapper for real-time auth events~~ Done 2026-09-24; `OnyxController` owns the org-wide subscription.

---

## Layer 4 — Core read screens

**What already exists (with path evidence):**

- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/DashboardScreen.kt` (131 lines) — real Compose screen: stat row (Missions/Tasks/Conflicts/Queued), error card, conflict-review warning, "Active missions" (first 3, "View all" switches bottom-nav), "Recent activity" (first 2 missions + first 2 tasks).
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/MissionsScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/TasksScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/MissionDetailScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/TaskDetailScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/NotificationsScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/controller/OnyxController.kt` — `refresh()` fans out 6 parallel `async`/`awaitAll` FFI calls: mission, task, approval, notification, sync status, conflicts. Every screen reads from already-loaded state.
- `mobile-android/app/src/main/kotlin/com/onyx/model/LoadedAggregate.kt` — 16-byte `ObjectId` array, `JSONObject` aggregate, `version`/`lifecycleEpoch`/`authorityEpoch`/`updatedAt` fields
- `mobile-android/app/src/main/kotlin/com/onyx/model/SyncSnapshot.kt` — sync status model
- `mobile-android/app/src/main/kotlin/com/onyx/model/CommandEnvelopeFactory.kt` — command construction
- `mobile-android/app/src/main/kotlin/com/onyx/model/SyncConflict.kt` — conflict model
- `mobile-android/app/src/main/kotlin/com/onyx/ui/AppShell.kt` — bottom-nav scaffold with 7 tabs

**Gaps:**
- `mobile_core_execute_query` JNI wrapper unwired at the Kotlin level (needed for detail-screen queries)
- `OnyxController`'s org-wide event subscription refreshes dashboards on `mission.event.`/`task.event.`/`notification.event.`; per-aggregate live-refresh call sites past that fan-out are future polish
- Screen-specific empty states need verification against `parity-matrix.md` § entries for each screen
- `NotificationsScreen` likely renders a bounded-context empty state (per DECISIONS.md M11-D8: "Notification local domain crates are not present")

**Next concrete task:**
1. Wire the real Kotlin query path over `nativeExecuteQuery` (Layer 2 task 1)
2. Verify each screen's empty/loading/error states match `parity-matrix.md` exactly
3. Add instrumented tests for Dashboard, Missions, TaskDetail screens on real device
4. Verify `OnyxController.refresh()` parallel fan-out produces correct results under concurrent load

---

## Layer 5 — Operational screens

**What already exists (with path evidence):**

- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/ApprovalsScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/FilesScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/SettingsScreen.kt` — exists
- `mobile-android/app/src/main/kotlin/com/onyx/ui/widgets/ConflictDialog.kt` — `Local`/`Remote`/`Escalate` resolution actions
- `mobile-android/app/src/main/kotlin/com/onyx/ui/widgets/SyncStatusIndicator.kt` — sync status widget
- `mobile-android/app/src/main/kotlin/com/onyx/background/BackgroundSync.kt` (30 lines) — schedules `WorkManagerService` every 15 min with `CONNECTED` network constraint, idempotent via `ExistingPeriodicWorkPolicy.KEEP`
- `mobile-android/app/src/main/kotlin/com/onyx/WorkManagerService.kt` — real `CoroutineWorker`
- `mobile-android/app/src/main/kotlin/com/onyx/background/BackgroundSync.kt` — `scheduleBackgroundSync()` called from `MainActivity.OnyxRoot`'s `LaunchedEffect`

**Gaps:**
- Event subscription is implemented and `OnyxController` refreshes with it; real-time conflict-resolution updates keyed off it are wired to the same fan-out, but conflict-specific live UI is hardware-verification pending
- Background sync behavior unverified on real Android (WorkManager constraints, Doze mode, battery optimization)
- File upload/download through `nativeUploadFile`/`nativeDownloadFile` unverified on real device (file permissions, storage access)
- Conflict resolution end-to-end (`nativeResolveConflict`) unverified on real device
- Secure session persistence uses Android Keystore (`SecureTokenStore.kt`) — the sanctioned path; no Rust binding exists or is planned (M11-D12.4)

**Next concrete task:**
1. ~~Add `mobile_core_subscribe_events` / `mobile_core_unsubscribe` JNI wrappers~~ Done 2026-09-24; `OnyxController.onCleared()` unsubscribes.
2. ~~Add `mobile_core_secure_storage` JNI wrappers if needed for session persistence~~ Resolved: not needed — Keystore.
3. Instrumented test: background sync scheduling → WorkManager execution → sync status update on real device
4. Instrumented test: file upload/download through JNI → mobile-core on real device
5. Instrumented test: conflict creation → resolution via `nativeResolveConflict` on real device

---

## Layer 6 — Android acceptance (OUTSTANDING GATE)

**This is the gate that was overridden in FLT-1. It must eventually close before any production mobile release.**

**What's missing:**
- **Real hardware testing.** No Android device has ever run the JNI → mobile-core → Rust chain. All instrumented tests (`androidTest/`) are currently either JVM unit tests or emulator-only.
- **P2P transport on device.** The **code subset is implemented** (Phase 4.1 + `DECISIONS P2P-11`): the Rust framing/encryption/handshake codec lives in `mobile-android-jni/src/p2p.rs` behind `com.onyx.p2p.P2pCodec`, the Kotlin `WifiDirectDriver`/`BleDriver`/`P2pChannel`/`P2pStream` stack is written, and the former placeholder C-ABI exports (`sync-transport-mobile`'s `android_wifi_direct`/`android_ble` + `mobile-core`'s re-export modules) were **deleted, not extended**. What remains is running it on two devices.
- **Background/offline execution.** `BackgroundSync.kt` schedules `WorkManagerService`; the scheduling contract is now asserted by `BackgroundSyncInstrumentedTest` (`androidTest`, hardware-gated) but has never been verified *running*: does WorkManager execute under Doze? Does the sync actually complete when offline? Does it retry correctly? Is the `REGISTERED_BACKGROUND_APP` global correctly managed across process lifecycle?
- **Accessibility.** No accessibility testing (TalkBack, switch access, font scaling).
- **Release artifact signing.** The Phase 4.5 code subset shipped the R8 setup (`proguard-rules.pro`, release buildType, CI `assembleRelease`) with debug-keystore signing; a production keystore and on-device survival check of the shrunk APK are outstanding.
- **Rollback artifact.** Per FLT-1, no Flutter APK exists to roll back to — this is the risk being accepted.

**Next concrete task:**
1. **(done, code subset)** The Kotlin→JNI P2P direction from `DECISIONS P2P-1` is implemented per `P2P-11` (codec + drivers landed, stubs deleted). Remainder: run the drivers' instrumented tests on two authorized Android devices.
2. Run `ONYX_MOBILE_DEVICE_TEST=1` instrumented tests on two authorized Android devices (including `BackgroundSyncInstrumentedTest` and a P2P session round-trip)
3. Verify background sync under Doze mode, airplane mode, and network switching
4. Verify `mobile_core_android_do_work` completes a real sync cycle on device
5. Add accessibility testing
6. Produce signed release APK with ProGuard rules
7. Add `mobile-android/app/src/androidTest/kotlin/com/onyx/` instrumented tests covering: full login, dashboard refresh, task workflow, approval flow, file upload/download, conflict resolution, background sync, P2P transport

---

## Dependency Order Summary

```
Layer 0 (done) ──→ Layer 1 (done) ──→ Layer 2 (mostly done) ──→ Layer 3 (partially done)
                                                                    │
                                                                    ▼
Layer 4 (partially done) ◄────────────────────────────────────────┘
                                                                     │
                                                                     ▼
Layer 5 (partially done) ◄────────────────────────────────────────┘
                                                                     │
                                                                     ▼
Layer 6 (code subset done; device gate OPEN) ───────────────────────┘
```

**Cross-cutting dependency:** Layer 6 requires all of Layers 1–5 to have instrumented tests that pass on real hardware. The P2P transport (Layer 6, item 1 above) is implemented as a code subset — Rust codec in the `mobile-android-jni` crate, Kotlin drivers in `mobile-android/` — and only the real-device half of the gate remains open.

---

## Source of Truth References

- **Frozen parity matrix:** `docs/mobile-migration/parity-matrix.md`
- **Flutter retirement decision:** `docs/DECISIONS.md` entry `FLT-1` (2026-09-24)
- **Flutter verification record:** `docs/MOBILE_V11_VERIFICATION.md`
- **Blueprint section 25:** `docs/governance/ONYX-MOB-01_Android_Kotlin_iOS_PWA_Technical_Blueprint_v1.1.md` §25 (A0–A6)
- **Mobile-core FFI exports:** `crates/mobile-core/src/lib.rs`, `ffi_mobile.rs`, `ffi_commands.rs`, `ffi_queries.rs`, `ffi_events.rs`, `ffi_files.rs`, `ffi_secure_storage.rs`
- **JNI adapter:** `crates/mobile-android-jni/src/lib.rs`
- **Kotlin app source:** `mobile-android/app/src/main/kotlin/com/onyx/`
- **Build config:** `mobile-android/app/build.gradle.kts`, `mobile-android/build.gradle.kts`
- **CI:** `.github/workflows/ci.yml` `mobile-android-kotlin` job
