#!/usr/bin/env bash
# Cross-compiles mobile-android-jni (and its mobile-core dependency) for
# every target Android ABI (ONYX-MOB-01 §3: arm64-v8a, armeabi-v7a,
# x86_64) into this module's jniLibs, mirroring mobile/tool/
# build_rust_android.sh's existing pattern for the frozen Flutter app
# rather than inventing a different one.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/mobile-android/app/src/main/jniLibs"

command -v cargo-ndk >/dev/null || cargo install cargo-ndk --locked
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android

mkdir -p "$OUT"
cd "$ROOT"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$OUT" build -p mobile-android-jni --release
