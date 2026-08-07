#!/usr/bin/env bash
set -euo pipefail
MOBILE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$MOBILE_ROOT"

if [[ -x android/gradlew && -f ios/Runner.xcodeproj/project.pbxproj ]]; then
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
for file in \
  android/app/src/main/java/com/onyx/MainActivity.kt \
  android/app/src/main/kotlin/com/onyx/WorkManagerService.kt \
  android/app/src/main/AndroidManifest.xml \
  ios/Runner/AppDelegate.swift \
  ios/Runner/BackgroundService.swift \
  ios/Runner/Info.plist \
  ios/Podfile \
  ios/mobile_core.podspec; do
  if [[ -f "$file" ]]; then
    mkdir -p "$tmp/$(dirname "$file")"
    cp "$file" "$tmp/$file"
  fi
done

flutter create --platforms=android,ios --org com.onyx --project-name onyx_mobile .

# The Flutter template creates a default MyApp widget test that does not
# describe ONYX and fails analysis after our real application entry point is
# restored. Keep only the repository-owned mobile tests.
rm -f test/widget_test.dart

while IFS= read -r -d '' file; do
  relative="${file#$tmp/}"
  mkdir -p "$(dirname "$relative")"
  cp "$file" "$relative"
done < <(find "$tmp" -type f -print0)

# Keep the frozen Android platform floor and package native Rust libraries
# from app/src/main/jniLibs. Flutter templates differ between releases, so
# patch both Groovy and Kotlin build files when present.
if [[ -f android/app/build.gradle ]]; then
  sed -i.bak -E 's/minSdkVersion flutter\.minSdkVersion/minSdkVersion 29/' android/app/build.gradle || true
  rm -f android/app/build.gradle.bak
fi
if [[ -f android/app/build.gradle.kts ]]; then
  sed -i.bak -E 's/minSdk = flutter\.minSdkVersion/minSdk = 29/' android/app/build.gradle.kts || true
  rm -f android/app/build.gradle.kts.bak
fi
