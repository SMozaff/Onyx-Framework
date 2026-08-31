# Report: Android Migration Work Packages H10 → M0 → A1 → A3 → A4

Covers the ordered sequence from ONYX-MOB-00 (Manifesto) / ONYX-MOB-01
(Blueprint): H10 (prerequisite hardening), M0 (freeze Flutter reference),
A1 (Kotlin skeleton + JNI adapter), A3 (login/session + secure token
storage), A4 (core screens). Full rationale for each is in `DECISIONS.md`
under the matching heading (`## H10`, `## H10.M0`, `## H10.A1`, `## H10.A3`,
`## H10.A4`); this file is the cross-task status/verification rollup.

## Status summary

| Task | Scope | Status | Commit | Merged to main |
|---|---|---|---|---|
| H10 | `client_type` enum enforcement + `mobile_observer` capability ceiling | Done | `3071797` | Yes, via PR #10 (`6266a1d`) |
| M0 | Freeze Flutter Android reference; parity matrix; freeze guard CI job | Done | `92c3f0e`, `825ddc2` | Yes, via PR #10 (`6266a1d`) |
| A1 | Kotlin Android skeleton + `mobile-android-jni` JNI adapter | Done | `8affc6c` | Yes, via PR #10 (`6266a1d`) |
| A3 | Kotlin login/session state machine + secure token storage | Done | `0319f06`, `d6ddb05` | Yes, via PR #10 (`6266a1d`) |
| A4 | Core screens: Dashboard, Missions, Tasks, Mission/Task Detail, Notifications | Code complete, tests passing | `2177d28` | **Pending** — PR #11 (draft), CI green except one re-running flaky job |

PR #10: https://github.com/SMozaff/Onyx-Framework/pull/10 (merged, merge commit `6266a1d`)
PR #11: https://github.com/SMozaff/Onyx-Framework/pull/11 (open, draft, head `2177d28`)

A blocker surfaced at M0's start: H10 was only on PR #10's unmerged branch,
not yet on `main`, though M0 requires starting at/after H10. Per the user's
choice, M0/A1/A3 were stacked directly on PR #10's branch rather than
waiting for a separate merge; PR #10 (H10+M0+A1+A3) was later merged to
`main` once its own CI was fully green. A4 then restarted the designated
branch from the merged `main` and became the new PR #11, per the project's
convention for a merged branch.

## H10 — client_type enum enforcement

- Closed the `client_type` enum and enforced a capability ceiling for the
  `mobile_observer` client type so a mobile client cannot exceed the
  authority a genuinely mobile-observer role should have.
- Verification: `cargo check --workspace`, `cargo clippy`, `cargo fmt`,
  and the full CI suite on PR #10 — all green.

## M0 — freeze Flutter Android reference

- Read the real, current Dart source (`mobile/lib/**`) directly rather
  than from memory, and built `docs/mobile-migration/parity-matrix.md` as
  the authoritative behavior reference for all later Kotlin ports —
  Dashboard, Missions, Tasks, Mission/Task Detail, Notifications,
  Approvals, shared-refresh architecture, command envelope shape.
- Added a `mobile-freeze-guard` CI job (green on every subsequent PR
  touching `mobile/`) and documented the freeze policy in
  `mobile/README.md`.

## A1 — Kotlin skeleton + JNI adapter

- New `mobile-android/` Gradle project (AGP 8.13.2, Kotlin 2.3.21,
  Compose BOM 2026.03.00, compileSdk/targetSdk 36, minSdk 29) and new
  `crates/mobile-android-jni` Rust crate.
- JNI wrappers `nativeNew`/`nativeFree`/`nativeExecuteCommand` against the
  `jni` crate's real 0.22.4 `EnvUnowned`/`Env` split (verified from the
  crate's own vendored source, since no external docs covered the new
  API shape).
- Cross-compiled via `cargo-ndk` to `aarch64-linux-android`,
  `armv7-linux-androideabi`, `x86_64-linux-android` (NDK
  28.2.13676358).
- Real host-JVM round-trip test against the linux-x86_64 `.so` caught a
  genuine UUID-encoding bug before it could reach a device: `MobileConfig
  .organization_id` is a raw 16-byte `ObjectId` array in the FFI JSON, not
  a UUID string.
- Verification: `cargo check -p mobile-android-jni`, `cargo clippy
  --all-targets -- -D warnings`, `cargo fmt --check`, `cargo check
  --workspace`, `./gradlew assembleDebug` — all clean/green.

## A3 — login/session state machine + secure token storage

- `OnyxSessionViewModel` (`OnyxUiState`: `Loading`/`NeedsLogin`/`Ready`/
  `StartupError`) as the Kotlin analog of Dart's session `ChangeNotifier`.
- `SecureTokenStore`: AES-256-GCM directly against the Android Keystore
  (`KeyGenParameterSpec`), not `EncryptedSharedPreferences` — confirmed
  deprecated as of `security-crypto:1.1.0-beta01` in favor of direct
  Keystore use.
- `AuthApi` (OkHttp) mirrors Dart's `OnyxHttpAuthApi`: login, hierarchy
  fetch, refresh, logout, including `MOBILE_ACCESS_RESTRICTED` handling.
- Found and fixed a second, distinct UUID-shape issue via a real failing
  test: `HierarchyUserWire.id` is a plain UUID **string**, unlike
  `MobileConfig.organization_id`'s byte-array shape — not assumed
  consistent by name similarity.
- Real bugs fixed during this task: an XML comment using `--` (invalid
  inside XML comments) broke `assembleDebug`; a Kotlin doc comment
  containing the literal substring `*.dart` opened an unintended nested
  block comment, leaving the real comment unclosed until EOF.
- Verification: `./gradlew assembleDebug`, `compileDebugKotlin` clean.

## A4 — core screens

- `OnyxController` (ViewModel + `StateFlow`) is the single shared source
  of truth every screen reads from, mirroring Dart's `OnyxController`
  `ChangeNotifier` — fans out the same six parallel FFI calls
  (`listAggregates` × mission/task/approval/notification,
  `getSyncStatus`, `listConflicts`) with all-or-nothing failure semantics
  and single-refresh-per-cycle behavior, preserved even for the
  `approval` fetch no current screen reads.
- Three new JNI wrappers (`nativeListAggregates`/`nativeGetSyncStatus`/
  `nativeListConflicts`) added to `mobile-android-jni`, sharing a new
  `copy_and_free_c_string` helper factored out of the existing
  `nativeExecuteCommand` logic.
- `CommandEnvelopeFactory` ports Dart's envelope construction field-for-
  field, including two real placeholders reproduced exactly rather than
  "fixed": the fixed `deviceId`, and the unsigned `Jwt` `authority_proof`.
- Real screens built: Dashboard, Missions (list + create), Mission Detail
  (Reject/Activate, gated on `status == "AwaitingApproval"`, Reject
  requiring a non-empty reason), Tasks (list + create, with a Snackbar
  instead of the create dialog when no missions exist), Task Detail
  (Reject/Approve, gated on `status == "Submitted"`), Notifications, all
  wired into a new `AppShell` bottom-nav shell — first four Dart nav
  destinations only; Approvals/Files/Settings deliberately out of scope
  (A5+).
- Real build/test fixes: `@OptIn(ExperimentalMaterial3Api::class)` for
  `Card(onClick=...)`/`TopAppBar`; replaced an unresolved
  `ExposedDropdownMenu` reference with a plain `DropdownMenu`; added
  `testImplementation("org.json:json:20250517")` because the Android SDK's
  `org.json` stub throws under plain JVM unit tests.
- 14 real, executed local JUnit tests (`UuidCodecTest`,
  `LoadedAggregateTest`/`SyncSnapshotTest`, `CommandEnvelopeFactoryTest`) —
  all passing.
- `OnyxControllerInstrumentedTest`: real, compiled (`compileDebugAndroidTestKotlin`
  succeeds), proves create→refresh, a real approve/reject status
  transition, and the single-refresh-per-cycle property via a
  `refreshCount` counter added specifically to make that property
  checkable. **Disclosed, not silently skipped**: never executed
  on-device — this sandbox has no `/dev/kvm` and reports zero
  `vmx`/`svm` CPU flags, so no Android emulator can boot, and no physical
  device was available.

### A4 verification table

| Check | Result |
|---|---|
| `cargo check -p mobile-android-jni` / `--workspace` | Clean |
| `cargo clippy -p mobile-android-jni --all-targets -- -D warnings` | Clean |
| `cargo fmt -p mobile-android-jni -- --check` | Clean |
| `./gradlew compileDebugKotlin` | Clean, zero warnings |
| `./gradlew assembleDebug` | `BUILD SUCCESSFUL`, real `app-debug.apk` with all 3 ABIs |
| `./gradlew testDebugUnitTest` | 14 passed, 0 failed |
| `./gradlew compileDebugAndroidTestKotlin` | Clean |
| `connectedAndroidTest` (real on-device run) | **Not run** — no KVM/emulator/device in this sandbox |
| PR #11 CI (`check`, `mobile-dart`, `deploy-check`, `web`, `mobile-android-kotlin`, `mobile-android`, `mobile-ios`, `mobile-freeze-guard`, `native-ui-evidence`) | Green on both triggered runs |
| PR #11 CI (`load-smoke`) | One run's job failed once on a transient p95-latency threshold flake (696ms vs 500ms; 0.28% error rate) unrelated to A4's Kotlin-only diff; the other run's identical job passed clean; failed job re-queued for re-run |

## Outstanding at time of writing

- PR #11 is still in **draft** and unmerged: CI is green on every job
  except one `load-smoke` re-run in flight. Once that comes back green,
  per the standing "commit, push and merge every update on `main`"
  instruction, the plan is: mark PR #11 ready for review, merge into
  `main`, unsubscribe from its activity, and report the merge commit hash.
- No task documents beyond A4 have been provided. A5 (Files, Settings,
  sync-conflict screens) and anything further are explicitly out of scope
  until requested.
