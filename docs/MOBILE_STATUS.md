# Mobile Status

- Rust FFI (`mobile-core`): ✅ Complete and extended for Flutter projections, sync status, conflict review, and background scheduling.
- Flutter UI (`mobile/`): ✅ Implemented for v1.1.
- Screens: ✅ Dashboard, Missions, Mission Detail, Tasks, Task Detail, Notifications, Approvals, Settings.
- Background sync: ✅ Source integration for iOS BGAppRefreshTask, Android WorkManager, and Dart background handlers.
- P2P integration: ✅ Routed through `mobile-core` / `sync-transport-mobile`; physical Wi-Fi Direct/BLE certification remains a device-lab gate because the inherited native stream placeholders currently return `ConnectionLost`.
- Runtime certification: ⏳ Run Flutter/Rust mobile CI on networked Android and macOS runners.
- Store distribution: ⏳ App Store / Play Store credentials and signing certificates remain operational go-live inputs.

## Required verification

```bash
mobile/tool/ensure_platform_scaffold.sh
mobile/tool/build_rust_android.sh
cd mobile && flutter pub get && flutter analyze && flutter test && flutter build apk

# macOS / Xcode
mobile/tool/build_rust_ios.sh
cd mobile && pod install --project-directory=ios && flutter build ios --simulator
```
