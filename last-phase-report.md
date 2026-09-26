# Last Phase Report — Wire the remaining Kotlin rows (query path + P2P controller)

**Date:** 2026-09-24
**Milestone:** `DECISIONS.md` **M11-D13**
**Branch:** `main`
**Commits:** `6ba4481` ("66", earlier changeset) + `4f6fbda` (review fixes)

## 1. Context

The Flutter→Kotlin Android migration (`docs/mobile-migration/`) had declared
nearly every JNI row wired. Two rows remained genuinely **unwired** and were
the last sandbox development work:

1. **`nativeExecuteQuery` at the Kotlin level** — a real query path
   (envelope builder + `OnyxController` call sites + detail-screen usage).
2. **P2P transport** — the Rust `P2pCodec` and the Kotlin drivers
   (`WifiDirectDriver`/`BleDriver`/`P2pChannel`/`P2pStream`) existed, but no
   controller owned them and nothing drove a session from the UI. (The
   earlier event-subscription milestone — real `nativeSubscribeEvents`/JNI +
   `EventCallback.kt` — had already been committed; see commits `0dd3338`
   through `6ba4481`.)

All prior verification of FFI signatures, JNI usage (jni 0.22), and header
predictions (`verify_ffi_signatures.sh`) was in place; compile/test is gated
entirely on GitHub Actions after push (no local build per directive). This
report covers the last phase in full.

## 2. Kotlin query path (contract row `nativeExecuteQuery` → real)

**Rust side (already present, this phase only consumes it):**
- `crates/mobile-core/src/ffi_queries.rs` — `mobile_core_execute_query(handle, query_json)`.
- `crates/applications/client-composition/src/app_state.rs` — `QueryRegistry`
  registers `GetMission`/`GetTask` (and `GetConversation`, `GetMessage`, …)
  against real repositories via `LoadAggregateHandler`.
- `query_registry.rs` — dispatch wraps a `Loaded` aggregate in the protocol
  response shape `LoadedJson = {aggregate, version, lifecycle_epoch,
  authority_epoch}`; a query for an unknown aggregate returns JSON `null`;
  an FFI `null` return means malformed input/dispatch error. The response
  carries **no** row `id` (only the `QueryEnvelope`'s `target_id` carries it)
  and no `updated_at`.

**Kotlin side (new this phase):**
- `mobile-android/app/src/main/kotlin/com/onyx/model/QueryEnvelopeFactory.kt` (new)
  — stateless builder producing exactly the registry's `QueryEnvelope`:
  `{"query_type": "GetMission"|"GetTask", "target_id": <16-byte array>}` via
  `UuidCodec.uuidToBytes`. Unlike `CommandEnvelopeFactory` it needs no
  org/user context.
- `OnyxController.kt` — added `suspend loadAggregateFromQuery(queryType,
  targetId): LoadedAggregate?` plus `loadMission`/`loadTask`:
  - builds the envelope, runs `MobileCoreBridge.nativeExecuteQuery` on
    `Dispatchers.Default`;
  - FFI `null` → `IllegalStateException` (mirrors the parent call's own
    contract: malformed/unknown query type);
  - JSON `"null"`/blank → aggregate-not-found → returns `null`;
  - re-injects the envelope's `target_id` bytes as the response's `id` so
    `LoadedAggregate.fromJson` (which expects the row shape `{id, aggregate,
    version, lifecycle_epoch, authority_epoch, updated_at}`) parses cleanly.
- `MissionDetailScreen.kt` / `TaskDetailScreen.kt` — on-open freshness pull:
  the screen keeps its frozen navigation snapshot but runs
  `LaunchedEffect(id) { runCatching { controller.loadMission/loadTask(id) }
  .getOrNull()?.let { fresh = it } }`. `canDecide`, title and the
  authority/execution cards all render `fresh`, so a decision is never built
  on data older than a fresh read. On failure the snapshot remains.

## 3. P2P controller + Settings surface (MIGRATION Phase 4.1 / Layer 5)

**`P2pController.kt` (new, ~306 lines)** — the missing call surface that owns
both media drivers:

- Public model: `P2pTransport` (`BLE` / `WIFI_DIRECT`), `P2pPeer(id, name,
  transport)`, sealed `P2pStatus` (`Idle`/`Starting`/`Advertising`/
  `Discovering`/`Connecting`/`Connected(peer)`/`Error(msg)`), `P2pMessage(text,
  incoming, timestamp)`.
- `StateFlow` surface: `status`, `peers`, `messages`.
- API: `permissionsFor(transport)`, `startServer`, `connectBle`,
  `discoverWifiPeers`, `connectWifi(peer)`, `send(text)`, `stop`, `release`;
  a single background executor (`"onyx-p2p"`, daemon) serializes session work.
- BLE is routed end-to-end (server = advertiser/responder, client =
  scanner/initiator); Wi-Fi Direct is discover → connect → group-formation
  poll (`GROUP_FORM_TIMEOUT_MS = 15s`) → owner TCP byte stream
  (`openStream` wrapped in `runCatching`).
- Both media converge on one `runHandshake(stream, initiator, peer)`:
  `P2pChannel(deferredHandshake = true)` + `setRawReader(HANDSHAKE_POINT_SIZE)`
  for the raw 65-byte codec public points, then `startFraming()`,
  `setOnMessage`, `send`. Handshake timeout `HANDSHAKE_TIMEOUT_MS = 10s`;
  `fail()` closes + nulls the channel and emits `P2pStatus.Error`.

**`P2pChannel.kt`** (modified, backward-compatible) — gained the deferred
handshake mode needed by the single-stream BLE medium:
- `deferredHandshake: Boolean = false` constructor param — when false (all
  prior callers), inbound bytes go straight into the frame buffer as before.
- `setRawReader(targetLength, onRaw)` — installs a raw message consumer
  before the session begins; **accumulation lives inside the channel** so
  arbitrary media chunking (BLE 20-byte MTU slices) assembles into complete
  65-byte records before `onRaw` fires.
- `startFraming()` — atomic switch back to frame accumulation once both
  public points have crossed; drains any raw-phase leftover into the frame
  buffer and extracts complete frames.

**`P2pViewModel.kt` (new)** — `AndroidViewModel` forwarding the controller's
`StateFlows`, calling `release()` in `onCleared()`; instantiated by the
default `AndroidViewModelFactory` via its `(Application)` constructor.

**`ui/widgets/P2pCard.kt` (new, ~180 lines)** — Settings card: transport
toggle, role controls (Start server / Scan & connect / Discover / Connect /
Stop), a runtime-permission request via `rememberLauncherForActivityResult(
RequestMultiplePermissions)` with a `pendingAction` guard for state change
recomposition, a probe composer + Send, and the message transcript.
Deliberately uses Material3 `TextField` — **not** `OutlinedTextField` — to
preserve the `SettingsScreenSourceTest` invariant (exactly one
`OutlinedTextField` in `SettingsScreen.kt`, bound to the relay field).

**Wiring:**
- `SettingsScreen.kt` — new trailing param `p2p: P2pViewModel? = null`; when
  non-null, inserts `P2pCard` after the Local-first database card (opt-in, so
  non-VM hosts and the source-level test are unaffected).
- `ui/AppShell.kt` — declares `p2p: P2pViewModel` and passes it to
  `SettingsScreen`.
- `MainActivity.kt` (Ready branch) — `val p2p: P2pViewModel = viewModel()`
  and hands it to `AppShell`.

## 4. Review fixes found in this phase (post-commit)

1. **BLE chunked handshake (correctness):** the first `P2pController` draft
   did raw-reader matching at the controller level, which required all 65
   bytes in a single read callback — impossible on BLE where GATT delivers
   ～20-byte MTU slices. Moved raw accumulation **inside `P2pChannel`**
   (`setRawReader(targetLength, onRaw)` advances as bytes arrive and emits
   exactly one complete 65-byte record; leftovers flow into the frame buffer
   at `startFraming`). Controller call site updated to the new signature.
2. **R8 keep for `P2pViewModel`:** reflectively instantiated by the default
   `AndroidViewModelFactory` — added `-keep class com.onyx.p2p.P2pViewModel
   { *; }` to `mobile-android/app/proguard-rules.pro` so the constructor and
   the whole P2P wiring survive R8 minification (the `mobile-android-kotlin`
   CI job builds `assembleRelease` with R8 and debug signing on every run).

Fix commit: `4f6fbda fix(P2P): accumulate raw handshake records over
chunked BLE; keep P2pViewModel from R8` — composed of `proguard-rules.pro`,
`P2pChannel.kt`, `P2pController.kt`.

## 5. Documentation updated

- `docs/DECISIONS.md` — appended **M11-D13** (query wiring + P2P controller),
  folding in corrections: the earlier "notification crates not present"
  premise is outdated (`crates/domains/notification-domain/` exists and is
  registered); PWA push client + delivery worker (`crates/bins/worker`) are
  confirmed built — nothing left to build there.
- `docs/mobile-migration/android-jni-contract.md` — `nativeExecuteQuery` row
  → implemented + wired; open item about the unwired query path struck; P2P
  section notes the codec is now driven.
- `docs/mobile-migration/KOTLIN_IMPLEMENTATION_PLAN.md` — Layer 2 row → "✅
  Declared and wired"; Layers 3/4 gaps struck; Layers 5/6 P2P files and
  statuses updated; NotificationsScreen empty-state note corrected.
- `docs/mobile-migration/MIGRATION_PLAN.md` — Phase 0.2 contract summary, the
  Phase 4.1 code-subset list (controller + view model + card now included),
  and a new **Phase 4.8** milestone recording query-path + P2P-controller
  completion and the still-open device gates.

## 6. Verification status

**CI-only by directive** (no local `cargo`/Gradle runs):

- `.github/workflows/ci.yml` `check` job: fmt → clippy → build → migrations →
  sqlx-cli verification (Rust).
- `mobile-android-kotlin` job (line 337): `cargo install cargo-ndk --locked`,
  cargo-ndk JNI cross-compile, `./gradlew assembleDebug` and
  `./gradlew assembleRelease` (R8 + debug signing), with APK artifacts
  uploaded.
- Manual gate (not in CI): `verify_ffi_signatures.sh` for cbindgen header
  drift.

**Blocked:** the push (and therefore CI) is blocked on GitHub credentials —
`GH_TOKEN` (env) is invalid, the keyring `gh` login is inactive, and no SSH
key is registered for the `https://github.com/SMozaff/Onyx-Framework.git`
remote. Commit `4f6fbda` is local-only until the credential is refreshed
(`gh auth login` / `gh auth refresh`).

## 7. Remaining gates (device/hardware — deliberately not in this phase)

- Two-device P2P session over real BLE + Wi-Fi Direct radios (BLE MTU and
  Wi-Fi Direct callback cadence; MIGRATION Phase 4.1b/d).
- Live `EventCallback` delivery on ART (JNI attach semantics on device).
- R8-shrunk APK survival on device; `connectedAndroidTest` instrumented
  suite; background/offline execution verification.
- Phase 5.4 hosting + CORS allow-list; Phase 6 GPG signing (human decisions).

## 8. File inventory

**New (this phase / review):**
- `mobile-android/app/src/main/kotlin/com/onyx/model/QueryEnvelopeFactory.kt`
- `mobile-android/app/src/main/kotlin/com/onyx/p2p/P2pController.kt`
- `mobile-android/app/src/main/kotlin/com/onyx/p2p/P2pViewModel.kt`
- `mobile-android/app/src/main/kotlin/com/onyx/ui/widgets/P2pCard.kt`

**Modified (this phase / review):**
- `mobile-android/app/src/main/kotlin/com/onyx/controller/OnyxController.kt`
  (query methods)
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/{MissionDetail,TaskDetail}Screen.kt`
  (freshness pulls)
- `mobile-android/app/src/main/kotlin/com/onyx/p2p/P2pChannel.kt`
  (deferred handshake + raw accumulation)
- `mobile-android/app/src/main/kotlin/com/onyx/ui/screens/SettingsScreen.kt`
- `mobile-android/app/src/main/kotlin/com/onyx/ui/AppShell.kt`
- `mobile-android/app/src/main/kotlin/com/onyx/MainActivity.kt`
- `mobile-android/app/proguard-rules.pro` (P2pViewModel keep)
- `docs/DECISIONS.md`, `docs/mobile-migration/android-jni-contract.md`,
  `docs/mobile-migration/KOTLIN_IMPLEMENTATION_PLAN.md`,
  `docs/mobile-migration/MIGRATION_PLAN.md`

**Invariants preserved:** `SettingsScreenSourceTest` (single
`OutlinedTextField`, relay-bound, `onSignOut`, no direct
`.organizationId =`/`.userId =`) is untouched; bridge method list is exactly
the 14 declarations in `MobileCoreBridge.kt`, each with at least one live
call site in the app; working tree is clean after the fix commit.