# ONYX Flutter Mobile v1.1 — Verification Record

**Document:** Mobile v1.1 delivery verification  
**Date:** 2026-08-05  
**Basis:** Frozen Team Prompt — Flutter Mobile App v1.0 and Ruling R5  
**Repository:** Updated ONYX workspace with the v1.1 mobile increment applied

## Delivery summary

The `mobile/` Flutter application and its supporting Rust/native integration have been implemented. The delivered source includes:

- Flutter application scaffold for Android and iOS.
- Dashboard, Missions, Mission Detail, Tasks, Task Detail, Notifications, Approvals, and Settings screens.
- A Dart bridge bound to the delivered `mobile-core` C ABI.
- Rust FFI extensions for aggregate lists, synchronization status, open conflict records, conflict resolution, and registered background synchronization.
- Sync-state and status widgets.
- Conflict comparison and Local/Remote/Escalate resolution actions.
- iOS `BGAppRefreshTask` registration and Android WorkManager integration.
- Mobile build helper scripts for Rust Android libraries and iOS XCFramework packaging.
- Unit, widget, bridge, navigation, background-sync, and device-lab P2P test sources.
- Mobile-specific GitHub Actions gates and offline verification scripts.

## Binding implementation decisions

The following gaps were resolved and recorded in `DECISIONS.md`:

1. The delivered Rust core exposes a raw C ABI rather than `flutter_rust_bridge` annotations. The Flutter bridge binds that authoritative ABI instead of creating a second execution core.
2. List projections and synchronization/conflict state remain Rust-owned. Dart renders returned projections and does not fabricate domain state.
3. Background schedulers call one process-registered `mobile-core` handle.
4. Cross-thread event strings are transferred to Dart ownership and explicitly freed after callback processing.
5. Notifications and Approvals render bounded-context empty states where the delivered local Rust composition has no corresponding aggregate implementation.
6. Platform runner scaffolding is completed or refreshed through `mobile/tool/ensure_platform_scaffold.sh` so generated Flutter/Xcode/Gradle files are not falsely treated as hand-authored domain artifacts.
7. Physical Wi-Fi Direct/BLE testing is opt-in because it requires two authorized devices and the delivered Rust transport byte-stream implementations are still placeholders.
8. No APK, IPA, `pubspec.lock`, or runtime test report was fabricated in an environment without the required toolchains.

## Verification executed in this environment

### Mobile static contract audit

Command:

```bash
python3 scripts/verify/verify_mobile_static.py
```

Result: **1,544 / 1,544 checks passed**.

Coverage includes:

- Required Flutter file manifest.
- Flutter/Dart source structure and delimiters.
- Relative Dart import resolution.
- FFI symbol correspondence between Dart, Rust, and the C header.
- Mobile-core projection, sync-status, conflict, resolution, and background APIs.
- Android manifest, Gradle, Kotlin wrapper, permissions, and WorkManager contracts.
- iOS plist, Podspec, Swift wrappers, BGTask registration, and Rust library linkage.
- Background-sync and event callback ownership contracts.
- UI screen/widget/test presence.
- CI mobile jobs and device-lab gate.
- Explicit P2P transport limitation disclosure.

Machine-readable result: `docs/MOBILE_V11_STATIC_REPORT.json`.

### Syntax and structural checks

The following checks passed:

- YAML parsing for Flutter configuration and modified GitHub Actions workflows.
- XML parsing for Android and iOS property files.
- Ruby syntax checks for the Podfile and podspec.
- Shell syntax checks for mobile build and verification scripts.
- Swift parser checks for `AppDelegate.swift` and `BackgroundService.swift`.
- Kotlin source-shape compilation against temporary Android/WorkManager stubs.
- Rust delimiter and exported-symbol checks for modified mobile-core files.
- Dart/TS-style balanced-delimiter and import audits for all mobile Dart sources.

### Existing platform regression audits

- Team 7 static verification: **120 checks passed**.
- Team 8 static verification: **346 checks passed**.

These are static/offline audits, not substitutes for Cargo, Flutter, Docker, or device execution.

## Acceptance status

| Criterion | Delivered source | Runtime certification |
|---|---:|---:|
| Android Flutter application | Yes | Pending networked Android runner/NDK |
| iOS Flutter application | Yes | Pending macOS/Xcode runner |
| FFI command/query/event bridge | Yes | Pending Flutter + Rust target execution |
| Dashboard and operational screens | Yes | Pending Flutter widget execution |
| Sync status states | Yes | Pending runtime event/sync exercise |
| Conflict dialog and resolution dispatch | Yes | Pending runtime conflict fixture |
| Android background sync | Yes | Pending emulator/device integration |
| iOS background sync | Yes | Pending simulator/device integration |
| Wi-Fi Direct/BLE P2P | Routed through Rust | **Blocked by delivered placeholder transport streams and physical-device requirement** |
| App Store/Play signing | Not a source-code task | Pending credentials/certificates |
| Mobile dependency lockfile | Not fabricated | Pending `flutter pub get` on approved networked runner |

## Runtime gates not executed here

This environment does not provide Flutter/Dart, Android SDK/NDK, Rust mobile targets, Xcode, CocoaPods, or a physical mobile device lab. Therefore the following commands remain required before final distribution certification:

```bash
# Android / Linux runner
mobile/tool/ensure_platform_scaffold.sh
mobile/tool/build_rust_android.sh
cd mobile
flutter pub get
flutter analyze
flutter test
flutter build apk

# iOS / macOS runner
mobile/tool/build_rust_ios.sh
cd mobile
pod install --project-directory=ios
flutter build ios --simulator
```

For the hardware-only transport gate:

```bash
cd mobile
ONYX_MOBILE_DEVICE_TEST=1 flutter test test/integration/p2p_sync_test.dart
```

That gate must remain blocked until the Wi-Fi Direct/BLE Rust `Connection` implementations no longer return `ConnectionLost` and two authorized devices are available.

## Remaining external release items

1. Generate and commit refreshed Rust and web/mobile dependency lockfiles in an approved networked environment.
2. Run GitHub Actions on the target repository.
3. Execute Android and iOS packaging gates.
4. Complete the native P2P transport byte-stream adapters and two-device validation.
5. Configure store credentials, signing certificates, provisioning profiles, and release metadata.

## Integrity statement

The delivered archives contain source, tests, configuration, and verification evidence only. They do not contain fabricated build outputs, lockfiles, signed store packages, or claims that unavailable runtime gates passed.
