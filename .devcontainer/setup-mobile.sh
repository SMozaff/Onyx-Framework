#!/usr/bin/env bash
# Installs Flutter, the Android SDK, and cargo-ndk inside the devcontainer.
#
# This mirrors .github/workflows/ci.yml's mobile-android/mobile-ios jobs
# (subosito/flutter-action@v2, channel: stable) rather than reinventing a
# separate recipe, so a Codespace and CI never drift onto different Flutter
# builds. It does NOT set FLUTTER_STORAGE_BASE_URL to a China mirror the way
# local setup on this project's original Windows machine needed to
# (see MEMORY.md: network-blocks-google-endpoints) — that workaround existed
# only because that specific network geo-blocked dl.google.com and
# storage.googleapis.com. GitHub-hosted Codespaces run on GitHub's own
# network and reach both directly; carrying the mirror workaround here would
# hide a real failure if this container is ever actually geo-blocked, so it
# is deliberately left out. If a future Codespaces region does hit the same
# block, that environment variable is the fix — see the memory file above
# for the exact values.
set -euo pipefail

echo "==> Installing Flutter (stable channel)"
if [ ! -d /opt/flutter ]; then
    sudo git clone --depth 1 --branch stable https://github.com/flutter/flutter.git /opt/flutter
    sudo chown -R "$(whoami)" /opt/flutter
fi
export PATH="/opt/flutter/bin:$PATH"
flutter --version
flutter config --no-analytics

echo "==> Installing Android SDK cmdline-tools"
ANDROID_HOME="/opt/android-sdk"
if [ ! -d "$ANDROID_HOME/cmdline-tools/latest" ]; then
    sudo mkdir -p "$ANDROID_HOME/cmdline-tools"
    sudo chown -R "$(whoami)" "$ANDROID_HOME"
    tmp="$(mktemp -d)"
    # Version pinned to match subosito/flutter-action's own default tooling
    # generation as of this repo's CI (ci.yml mobile-android job) — bump
    # both together if that job's cmdline-tools version ever changes.
    curl -fsSL -o "$tmp/cmdline-tools.zip" \
        "https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip"
    unzip -q "$tmp/cmdline-tools.zip" -d "$tmp"
    mv "$tmp/cmdline-tools" "$ANDROID_HOME/cmdline-tools/latest"
    rm -rf "$tmp"
fi
export ANDROID_HOME PATH="$ANDROID_HOME/cmdline-tools/latest/bin:$ANDROID_HOME/platform-tools:$PATH"

echo "==> Accepting Android SDK licenses and installing platform/build-tools"
yes | sdkmanager --licenses >/dev/null
sdkmanager --install \
    "platform-tools" \
    "platforms;android-34" \
    "build-tools;34.0.0" \
    "ndk;26.1.10909125"

echo "==> Installing cargo-ndk (Team Prompt 5 mobile-core cross-compile)"
command -v cargo-ndk >/dev/null || cargo install cargo-ndk --locked
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
# aarch64-apple-ios deliberately NOT added here: rustup would happily install
# the target triple, but this is a Linux container with no Apple SDK/linker,
# so a build against it fails regardless — the rustup step would create a
# false sense of readiness. CI's own mobile-ios job runs on macos-latest
# (ci.yml), confirmed rather than assumed before writing this note. iOS
# builds need an actual Mac — this container does not attempt to fake one.

echo "==> Fetching Flutter package dependencies"
(cd mobile && flutter pub get)

echo "==> Mobile toolchain ready."
