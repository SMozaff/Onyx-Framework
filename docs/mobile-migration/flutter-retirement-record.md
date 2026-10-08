# Flutter Retirement Record

**Document:** `docs/mobile-migration/flutter-retirement-record.md`
**Date:** 2026-09-23
**Status:** HISTORICAL RECORD — DO NOT REWRITE AS IF THE GATE PASSED

## Verdict

The Flutter retirement gate was **not met** when Flutter was deleted. Deletion proceeded under an explicit owner override. This file records that decision as an **overridden gate**, not a passed gate.

Canonical decision: `docs/DECISIONS.md`, entry `FLT-1`, dated 2026-09-24.

## Unmet retirement conditions

`docs/MOBILE_V11_VERIFICATION.md` records all runtime gates as pending:

- JNI/mobile FFI had never been run on a real Android/iOS device.
- Wi-Fi Direct/BLE Rust transport byte-stream work was blocked by placeholder transport implementations and by the physical-device requirement.
- Android WorkManager and iOS background-sync behavior were present in source but pending emulator/device integration.
- No APK, IPA, lockfile, or runtime test report had been fabricated or committed; there was no shipped Flutter binary to roll back from.

## Deletion performed

- Deleted `mobile/` in its entirety, including Flutter application source, Android/iOS platform wrappers, tests, build tooling, `pubspec.yaml`, analysis options, freeze-exception log, mobile README, and mobile `.gitignore`.
- Removed these CI jobs from `.github/workflows/ci.yml`:
  - `mobile-freeze-guard`
  - `mobile-dart`
  - `mobile-android`
  - `mobile-ios`
- Deleted these Flutter-specific verification scripts:
  - `scripts/verify/verify_mobile.sh`
  - `scripts/verify/verify_mobile_freeze.sh`
  - `scripts/verify/verify_mobile_static.py`
- Updated `README.md` so Flutter is described as retired rather than currently active.
- Preserved `mobile-android/`, `crates/mobile-core/`, and `crates/mobile-android-jni/`.
- `mobile-pwa/` did not exist and no action was taken for it.
- Preserved historical records: `docs/MOBILE_V11_VERIFICATION.md`, `docs/MOBILE_V11_STATIC_REPORT.json`, `docs/MOBILE_V11_CHANGED_FILES.txt`, `docs/mobile-migration/parity-matrix.md`, and historical Flutter-era `DECISIONS.md` entries.

## Consequence

Until Kotlin Android acceptance closes, ONYX has no fully proven mobile client. The Kotlin implementation plan must still close that outstanding acceptance gate before any production mobile release.
