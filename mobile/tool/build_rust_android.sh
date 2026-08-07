#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
OUT="$ROOT/mobile/android/app/src/main/jniLibs"
command -v cargo-ndk >/dev/null || cargo install cargo-ndk --locked
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
mkdir -p "$OUT"
cd "$ROOT"
cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o "$OUT" build -p mobile-core --release
