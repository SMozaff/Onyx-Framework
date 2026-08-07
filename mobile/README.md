# ONYX Mobile v1.1

Flutter UI for the existing `mobile-core` Rust library.

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
