#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
FRAMEWORK="$ROOT/mobile/ios/Frameworks/MobileCore.xcframework"
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios
cd "$ROOT"
cargo build -p mobile-core --release --target aarch64-apple-ios
cargo build -p mobile-core --release --target aarch64-apple-ios-sim
cargo build -p mobile-core --release --target x86_64-apple-ios
mkdir -p "$ROOT/mobile/ios/Frameworks" /tmp/onyx-ios-sim
lipo -create \
  target/aarch64-apple-ios-sim/release/libmobile_core.a \
  target/x86_64-apple-ios/release/libmobile_core.a \
  -output /tmp/onyx-ios-sim/libmobile_core.a
rm -rf "$FRAMEWORK"
xcodebuild -create-xcframework \
  -library target/aarch64-apple-ios/release/libmobile_core.a -headers crates/mobile-core \
  -library /tmp/onyx-ios-sim/libmobile_core.a -headers crates/mobile-core \
  -output "$FRAMEWORK"
