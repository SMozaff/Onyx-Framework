# ONYX Mobile v1.1

Flutter UI for the existing `mobile-core` Rust library.

## ⚠ Frozen Reference Implementation (M0)

`mobile/lib/` is frozen (ONYX-MOB-00 §8 / `DECISIONS.md`'s M0 entry) while
the native Kotlin Android rewrite (`mobile-android/`) and the iOS
Observer PWA (`mobile-pwa/`) are built. **No ordinary new product
development happens here anymore.** Only real security fixes and
critical defects may still land, and every such change must also edit
[`FROZEN_EXCEPTION.md`](./FROZEN_EXCEPTION.md) in the same commit,
stating what was fixed and why it qualifies. This is enforced in CI —
`mobile-freeze-guard` (`.github/workflows/ci.yml`,
`scripts/verify/verify_mobile_freeze.sh`) fails a diff that touches
`mobile/lib/` without that file also being touched — not just a written
policy.

The precise, current behavior this app must not regress is documented in
[`docs/mobile-migration/parity-matrix.md`](../docs/mobile-migration/parity-matrix.md) —
the real acceptance criteria the Kotlin rewrite is built against.

## Development

```bash
cd mobile
flutter pub get
flutter analyze
flutter test
```

## Native Rust libraries

Android:

```bash
mobile/tool/build_rust_android.sh
flutter build apk
```

iOS (macOS/Xcode required):

```bash
mobile/tool/build_rust_ios.sh
flutter build ios --no-codesign
```

The bridge binds the cbindgen ABI delivered by `crates/mobile-core`. It does not duplicate domain validation in Dart. Mission and Task data are read from the local SQLite projection through Rust; sync status and conflict decisions use the real `SyncAgent` and `ConflictRecord` types.
