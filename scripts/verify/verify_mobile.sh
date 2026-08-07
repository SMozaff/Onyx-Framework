#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

required=(
  mobile/pubspec.yaml
  mobile/lib/main.dart
  mobile/lib/bridge/bridge.dart
  mobile/lib/ui/app.dart
  mobile/lib/ui/screens/dashboard.dart
  mobile/lib/ui/screens/missions.dart
  mobile/lib/ui/screens/mission_detail.dart
  mobile/lib/ui/screens/tasks.dart
  mobile/lib/ui/screens/task_detail.dart
  mobile/lib/ui/screens/notifications.dart
  mobile/lib/ui/screens/approvals.dart
  mobile/lib/ui/screens/settings.dart
  mobile/lib/ui/widgets/sync_status.dart
  mobile/lib/ui/widgets/conflict_dialog.dart
  mobile/android/app/src/main/java/com/onyx/MainActivity.kt
  mobile/android/app/src/main/kotlin/com/onyx/WorkManagerService.kt
  mobile/ios/Runner/AppDelegate.swift
  mobile/ios/Runner/BackgroundService.swift
)
for file in "${required[@]}"; do
  [[ -f "$file" ]] || { echo "missing $file" >&2; exit 1; }
done

grep -q 'mobile_core_execute_command' mobile/lib/bridge/bridge.dart
grep -q 'mobile_core_get_sync_status' mobile/lib/bridge/bridge.dart
grep -q 'mobile_core_list_conflicts' mobile/lib/bridge/bridge.dart
grep -q 'mobile_core_resolve_conflict' mobile/lib/bridge/bridge.dart
grep -q 'BGTaskScheduler' mobile/ios/Runner/BackgroundService.swift
grep -q 'CoroutineWorker' mobile/android/app/src/main/kotlin/com/onyx/WorkManagerService.kt
grep -q 'Wi-Fi Direct or BLE sync completes' mobile/test/integration/p2p_sync_test.dart
grep -q 'ONYX_MOBILE_DEVICE_TEST' mobile/test/integration/p2p_sync_test.dart

echo "Mobile structural contract verified"
