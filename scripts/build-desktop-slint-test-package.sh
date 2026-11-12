#!/usr/bin/env bash
# Builds an installable TEST BUILD .deb for the desktop-slint feasibility host.
# Not a production release. Usage:
#   build-desktop-slint-test-package.sh <binary> <version> <commit-sha> <api-origin> <outdir>
set -euo pipefail

binary="$1"; version="$2"; commit="$3"; api_origin="$4"; outdir="$5"
pkg="onyx-desktop-slint-test"
arch="$(dpkg --print-architecture)"
[[ "$arch" == "amd64" ]] || { echo "::error::Expected amd64 build host, got $arch"; exit 1; }
[[ -x "$binary" ]] || { echo "::error::Binary not found: $binary"; exit 1; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
root="$work/$pkg"
mkdir -p "$root/DEBIAN" "$root/usr/bin" \
  "$root/usr/share/applications" "$root/usr/share/doc/$pkg"

install -m 0755 "$binary" "$root/usr/bin/$pkg"

cat > "$root/usr/share/applications/$pkg.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=ONYX Desktop Slint (TEST BUILD)
Comment=TEST BUILD / NOT FOR PRODUCTION. Feasibility host.
Exec=$pkg
Terminal=false
Categories=Utility;
DESKTOP

cat > "$root/usr/share/doc/$pkg/BUILD-INFO.txt" <<INFO
ONYX Desktop Slint: TEST BUILD / NOT FOR PRODUCTION
Package: $pkg
Version: $version
Source commit: $commit
API origin (build-time): $api_origin
Target: Debian 13 (trixie), amd64
INFO

# Runtime dependencies come from the binary's actual shared-library links.
shlib_dir="$work/shlibs"
mkdir -p "$shlib_dir/debian"
printf 'Source: %s\n\nPackage: %s\nArchitecture: any\nDescription: shlib probe\n' "$pkg" "$pkg" > "$shlib_dir/debian/control"
depends="$(cd "$shlib_dir" && dpkg-shlibdeps -O "$root/usr/bin/$pkg" 2>/dev/null | sed -n 's/^shlibs:Depends=//p')"
[[ -n "$depends" ]] || { echo "::error::Could not derive runtime dependencies"; exit 1; }
# winit, Slint and the GL stack load these at runtime (dlopen), so dpkg-shlibdeps cannot see them.
# Keep this list in step with: strings target/release/desktop-slint | grep '\.so'
runtime_libs="libegl1, libgl1, libx11-6, libx11-xcb1, libxcb1, libxcursor1, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0"
depends="$depends, $runtime_libs"

installed_size="$(du -sk "$root" | cut -f1)"
cat > "$root/DEBIAN/control" <<CONTROL
Package: $pkg
Version: $version
Architecture: amd64
Maintainer: ONYX test builds <noreply@invalid>
Installed-Size: $installed_size
Depends: $depends
Section: utils
Priority: optional
Description: ONYX Desktop Slint feasibility host (TEST BUILD / NOT FOR PRODUCTION)
 Test artifact for owner evaluation. Not a production release and not a
 replacement for the Tauri desktop application.
CONTROL

mkdir -p "$outdir"
deb="$outdir/${pkg}_${version}_amd64.deb"
dpkg-deb --root-owner-group --build "$root" "$deb" >/dev/null
echo "$deb"
