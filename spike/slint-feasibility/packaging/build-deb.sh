#!/usr/bin/env bash
# Builds a .deb for the Linux spike binary. Usage: build-deb.sh <binary> <out-dir>
set -euo pipefail

BIN="${1:?path to release binary}"
OUT="${2:?output directory}"
PKG="onyx-slint-spike"
VERSION="0.1.0"
ARCH="amd64"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT

install -Dm755 "$BIN" "$STAGE/usr/bin/$PKG"

install -d "$STAGE/usr/share/doc/$PKG"
cat > "$STAGE/usr/share/doc/$PKG/README" <<DOC
ONYX Slint feasibility spike (test build, not for end users).
DOC

install -d "$STAGE/DEBIAN"
cat > "$STAGE/DEBIAN/control" <<CONTROL
Package: $PKG
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: ONYX <noreply@example.invalid>
Depends: libc6, libfontconfig1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libxcb1
Description: ONYX Slint feasibility spike
 Test window for the Slint migration spike. Not a product release.
CONTROL

mkdir -p "$OUT"
dpkg-deb --root-owner-group --build "$STAGE" "$OUT/${PKG}_${VERSION}-1_${ARCH}.deb"
dpkg-deb --info "$OUT/${PKG}_${VERSION}-1_${ARCH}.deb" | head -20
