#!/usr/bin/env python3
"""Verify ONYX brand icon transparency, dimensions, source mapping, and references."""
from __future__ import annotations

import importlib.util
import json
import re
from pathlib import Path
from PIL import Image, ImageChops

STANDARD_RING_PNGS = {
    "assets/branding/onyx-ring.png",
    "crates/bins/desktop-shell/icons/icon.png",
    "crates/bins/desktop-shell/ui/public/favicon.png",
    "crates/bins/desktop-shell/ui/public/onyx-ring.png",
    "web-ui/public/favicon.png",
    "mobile-pwa/public/icons/icon-192.png",
    "mobile-pwa/public/icons/icon-512.png",
    "mobile-pwa/public/icons/icon-192-maskable.png",
    "mobile-pwa/public/icons/icon-512-maskable.png",
    "mobile-pwa/public/icons/apple-touch-icon.png",
}

ROOT = Path(__file__).resolve().parents[2]
_generator_path = ROOT / "scripts/branding/generate-native-icons.py"
_spec = importlib.util.spec_from_file_location("onyx_icon_generator", _generator_path)
if _spec is None or _spec.loader is None:
    raise RuntimeError(f"cannot load icon generator at {_generator_path}")
_generator = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_generator)
BRANDS = _generator.BRANDS
load_square = _generator.load_square
resized = _generator.resized
padded_maskable = _generator.padded_maskable

ERRORS: list[str] = []
CHECKED: dict[str, tuple[int, int, float]] = {}
EXPECTED_PNGS: dict[str, tuple[int | None, int | None]] = {
    "assets/branding/onyx-ring.png": (1024, 1024),
    "assets/branding/admin-shell.png": (1024, 1024),
    "crates/bins/admin-shell/icons/icon.png": (1024, 1024),
    "crates/bins/admin-shell/ui/public/favicon.png": (96, 96),
    "crates/bins/desktop-shell/icons/icon.png": (1024, 1024),
    "crates/bins/desktop-shell/ui/public/favicon.png": (96, 96),
    "crates/bins/desktop-shell/ui/public/onyx-ring.png": (1024, 1024),
    "crates/bins/desktop-shell/ui/src/assets/onyx-logo-horizontal.png": (None, None),
    "web-ui/public/favicon.png": (96, 96),
    "mobile-pwa/public/icons/icon-192.png": (192, 192),
    "mobile-pwa/public/icons/icon-512.png": (512, 512),
    "mobile-pwa/public/icons/icon-192-maskable.png": (192, 192),
    "mobile-pwa/public/icons/icon-512-maskable.png": (512, 512),
    "mobile-pwa/public/icons/apple-touch-icon.png": (180, 180),
    "mobile-android/app/src/main/res/mipmap-mdpi/ic_launcher.png": (48, 48),
    "mobile-android/app/src/main/res/mipmap-mdpi/ic_launcher_round.png": (48, 48),
    "mobile-android/app/src/main/res/mipmap-hdpi/ic_launcher.png": (72, 72),
    "mobile-android/app/src/main/res/mipmap-hdpi/ic_launcher_round.png": (72, 72),
    "mobile-android/app/src/main/res/mipmap-xhdpi/ic_launcher.png": (96, 96),
    "mobile-android/app/src/main/res/mipmap-xhdpi/ic_launcher_round.png": (96, 96),
    "mobile-android/app/src/main/res/mipmap-xxhdpi/ic_launcher.png": (144, 144),
    "mobile-android/app/src/main/res/mipmap-xxhdpi/ic_launcher_round.png": (144, 144),
    "mobile-android/app/src/main/res/mipmap-xxxhdpi/ic_launcher.png": (192, 192),
    "mobile-android/app/src/main/res/mipmap-xxxhdpi/ic_launcher_round.png": (192, 192),
}


def fail(message: str) -> None:
    ERRORS.append(message)


def require_transparent_alpha(image: Image.Image, label: str) -> float:
    rgba = image.convert("RGBA")
    alpha = rgba.getchannel("A")
    histogram = alpha.histogram()
    total = rgba.width * rgba.height
    transparent_fraction = histogram[0] / total
    if alpha.getextrema()[0] != 0 or histogram[0] == 0:
        fail(f"{label}: no fully transparent pixels (background may be baked in)")
    if transparent_fraction < 0.05:
        fail(f"{label}: only {transparent_fraction:.2%} fully transparent pixels; expected >= 5%")
    corners = {
        "top-left": alpha.getpixel((0, 0)),
        "top-right": alpha.getpixel((rgba.width - 1, 0)),
        "bottom-left": alpha.getpixel((0, rgba.height - 1)),
        "bottom-right": alpha.getpixel((rgba.width - 1, rgba.height - 1)),
    }
    opaque_corners = [name for name, value in corners.items() if value != 0]
    if opaque_corners:
        fail(f"{label}: background corners are not transparent: {', '.join(opaque_corners)}")
    bounds = alpha.getbbox()
    if bounds is None:
        fail(f"{label}: image is blank/fully transparent")
    else:
        left, top, right, bottom = bounds
        if left == 0 or top == 0 or right >= rgba.width or bottom >= rgba.height:
            fail(f"{label}: visible logo touches an image edge and may be clipped (alpha bounds {bounds})")
        visible_fraction = (right - left) * (bottom - top) / total
        if visible_fraction < 0.01:
            fail(f"{label}: visible artwork is unexpectedly small ({visible_fraction:.2%} of canvas)")
    return transparent_fraction


def validate_png(relative: str, expected: tuple[int | None, int | None]) -> Image.Image | None:
    path = ROOT / relative
    if not path.is_file():
        fail(f"{relative}: referenced/required file is missing")
        return None
    try:
        with Image.open(path) as original:
            fmt = original.format
            original_mode = original.mode
            has_transparency = original_mode in ("RGBA", "LA") or "transparency" in original.info
            if fmt != "PNG":
                fail(f"{relative}: actual format is {fmt!r}, not PNG")
            if not has_transparency:
                fail(f"{relative}: encoded PNG has no alpha/transparency channel")
            image = original.convert("RGBA")
            want_w, want_h = expected
            if want_w is not None and image.size != (want_w, want_h):
                fail(f"{relative}: dimensions {image.size}, expected {(want_w, want_h)}")
            if image.getbbox() is None:
                fail(f"{relative}: decoded pixels are blank")
            ratio = require_transparent_alpha(image, relative)
            if relative in STANDARD_RING_PNGS or relative.startswith("mobile-android/app/src/main/res/mipmap-"):
                center_alpha = image.getchannel("A").getpixel((image.width // 2, image.height // 2))
                if center_alpha != 0:
                    fail(f"{relative}: standard ONYX ring center is not transparent (alpha={center_alpha}); a black background may be baked into the logo hole")
            CHECKED[relative] = (image.width, image.height, ratio)
            return image.copy()
    except Exception as error:
        fail(f"{relative}: cannot decode PNG ({error})")
        return None


def assert_matches(relative: str, actual: Image.Image | None, expected: Image.Image) -> None:
    if actual is None:
        return
    if actual.size != expected.size:
        fail(f"{relative}: generated dimensions {actual.size} do not match canonical-derived size {expected.size}")
        return
    difference = ImageChops.difference(actual.convert("RGBA"), expected.convert("RGBA"))
    if any(high != 0 for _low, high in difference.getextrema()):
        fail(f"{relative}: pixels are stale or do not derive from the designated canonical PNG")


def validate_tauri_containers() -> None:
    required_ico_sizes = {16, 24, 32, 48, 64, 128, 256}
    # ICNS stores Retina variants as (logical size, scale=2), e.g. 512@2x
    # represents a 1024x1024 source without an additional 1024 logical-size entry.
    required_icns_sizes = {128, 256, 512}
    for _name, crate in BRANDS.items():
        ico_path = crate / "icons/icon.ico"
        icns_path = crate / "icons/icon.icns"
        if not ico_path.is_file():
            fail(f"{ico_path.relative_to(ROOT)}: required Tauri ICO derivative is missing")
        else:
            try:
                with Image.open(ico_path) as ico:
                    if ico.format != "ICO":
                        fail(f"{ico_path.relative_to(ROOT)}: container format is {ico.format!r}, expected ICO")
                    sizes = {size[0] for size in ico.ico.sizes()}
                    if not required_ico_sizes.issubset(sizes):
                        fail(f"{ico_path.relative_to(ROOT)}: ICO sizes {sorted(sizes)}; missing {sorted(required_ico_sizes - sizes)}")
                    for size in required_ico_sizes & sizes:
                        layer = ico.ico.getimage((size, size)).convert("RGBA")
                        require_transparent_alpha(layer, f"{ico_path.relative_to(ROOT)} [{size}x{size}]")
            except Exception as error:
                fail(f"{ico_path.relative_to(ROOT)}: cannot validate ICO ({error})")
        if not icns_path.is_file():
            fail(f"{icns_path.relative_to(ROOT)}: required Tauri ICNS derivative is missing")
        else:
            try:
                with Image.open(icns_path) as icns:
                    if icns.format != "ICNS":
                        fail(f"{icns_path.relative_to(ROOT)}: container format is {icns.format!r}, expected ICNS")
                    sizes = {size[0] for size in icns.info.get("sizes", [])}
                    if not required_icns_sizes.issubset(sizes):
                        fail(f"{icns_path.relative_to(ROOT)}: ICNS sizes {sorted(sizes)}; expected at least {sorted(required_icns_sizes)}")
                    require_transparent_alpha(icns.convert("RGBA"), f"{icns_path.relative_to(ROOT)} [decoded image]")
            except Exception as error:
                fail(f"{icns_path.relative_to(ROOT)}: cannot validate ICNS ({error})")


def validate_references() -> None:
    html_icons = {
        "web-ui/index.html": ROOT / "web-ui/public",
        "mobile-pwa/index.html": ROOT / "mobile-pwa/public",
        "crates/bins/admin-shell/ui/index.html": ROOT / "crates/bins/admin-shell/ui/public",
        "crates/bins/desktop-shell/ui/index.html": ROOT / "crates/bins/desktop-shell/ui/public",
    }
    for html_relative, public_root in html_icons.items():
        html_path = ROOT / html_relative
        if not html_path.is_file():
            fail(f"{html_relative}: HTML entry point is missing")
            continue
        content = html_path.read_text(encoding="utf-8")
        for tag in re.findall(r"<link\b[^>]*>", content, flags=re.IGNORECASE):
            rel_match = re.search(r"\brel\s*=\s*[\"']([^\"']+)", tag, flags=re.IGNORECASE)
            href_match = re.search(r"\bhref\s*=\s*[\"']([^\"']+)", tag, flags=re.IGNORECASE)
            if not rel_match or not href_match:
                continue
            relations = rel_match.group(1).lower().split()
            href = href_match.group(1)
            if "icon" not in relations and "apple-touch-icon" not in relations:
                continue
            if "://" in href or href.startswith("data:"):
                continue
            relative_href = href.lstrip("/") if href.startswith("/") else href
            target = public_root / relative_href
            if not target.is_file():
                fail(f"{html_relative}: icon reference {href!r} does not resolve to {target.relative_to(ROOT)}")

    # Tauri resolves bundle.icon entries relative to each shell's config directory.
    for _name, crate in BRANDS.items():
        conf_path = crate / "tauri.conf.json"
        try:
            config = json.loads(conf_path.read_text(encoding="utf-8"))
            configured = config["bundle"]["icon"]
            for entry in configured:
                target = crate / entry
                if not target.is_file():
                    fail(f"{conf_path.relative_to(ROOT)}: bundle icon {entry!r} does not exist")
            if "icons/icon.png" not in configured:
                fail(f"{conf_path.relative_to(ROOT)}: bundle.icon must include icons/icon.png")
        except Exception as error:
            fail(f"{conf_path.relative_to(ROOT)}: cannot validate Tauri icon config ({error})")

    manifest_relative = "mobile-pwa/public/manifest.webmanifest"
    try:
        manifest = json.loads((ROOT / manifest_relative).read_text(encoding="utf-8"))
        declared = set()
        for entry in manifest.get("icons", []):
            href = entry.get("src", "")
            path = ROOT / "mobile-pwa/public" / href.lstrip("/")
            if not path.is_file():
                fail(f"{manifest_relative}: icon src {href!r} does not exist")
                continue
            declared.add(path.relative_to(ROOT).as_posix())
            try:
                with Image.open(path) as image:
                    actual_size = f"{image.width}x{image.height}"
                    if actual_size not in entry.get("sizes", "").split():
                        fail(f"{manifest_relative}: {href!r} declares sizes {entry.get('sizes')!r}, actual {actual_size}")
                    if entry.get("type") != "image/png" or image.format != "PNG":
                        fail(f"{manifest_relative}: {href!r} must be declared and encoded as image/png")
                    expected_purpose = "maskable" if "maskable" in path.name else "any"
                    if expected_purpose not in entry.get("purpose", "any").split():
                        fail(f"{manifest_relative}: {href!r} purpose does not include {expected_purpose!r}")
            except Exception as error:
                fail(f"{manifest_relative}: cannot inspect {href!r} ({error})")
        needed = {
            "mobile-pwa/public/icons/icon-192.png",
            "mobile-pwa/public/icons/icon-512.png",
            "mobile-pwa/public/icons/icon-192-maskable.png",
            "mobile-pwa/public/icons/icon-512-maskable.png",
        }
        if not needed.issubset(declared):
            fail(f"{manifest_relative}: missing manifest entries for {sorted(needed - declared)}")
    except Exception as error:
        fail(f"{manifest_relative}: cannot parse manifest ({error})")

    android_manifest = ROOT / "mobile-android/app/src/main/AndroidManifest.xml"
    if not android_manifest.is_file():
        fail("mobile-android/app/src/main/AndroidManifest.xml: missing Android manifest")
    else:
        content = android_manifest.read_text(encoding="utf-8")
        for resource in ("ic_launcher", "ic_launcher_round"):
            if f"@mipmap/{resource}" not in content:
                fail(f"AndroidManifest.xml: launcher reference @mipmap/{resource} is missing")
            for density in ("mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"):
                path = ROOT / f"mobile-android/app/src/main/res/mipmap-{density}/{resource}.png"
                if not path.is_file():
                    fail(f"Android launcher resource is missing: {path.relative_to(ROOT)}")

    login_relative = "crates/bins/desktop-shell/ui/src/pages/Login.tsx"
    login = ROOT / login_relative
    if not login.is_file():
        fail(f"{login_relative}: missing")
    else:
        source = login.read_text(encoding="utf-8")
        if 'src="/onyx-ring.png"' not in source:
            fail(f"{login_relative}: must reference the generated full-resolution standard /onyx-ring.png")
        if "onyx-logo.png" in source:
            fail(f"{login_relative}: stale import of the wordmark asset remains")
        if not (ROOT / "crates/bins/desktop-shell/ui/public/onyx-ring.png").is_file():
            fail("Staff login full-resolution ring is missing from desktop-shell UI public assets")


def validate_generated_mappings() -> None:
    ring = load_square(ROOT / "assets/branding/onyx-ring.png")
    admin = load_square(ROOT / "assets/branding/admin-shell.png")
    expected: dict[str, Image.Image] = {}
    expected["crates/bins/admin-shell/icons/icon.png"] = resized(admin, 1024)
    expected["crates/bins/admin-shell/ui/public/favicon.png"] = resized(admin, 96)
    expected["crates/bins/desktop-shell/icons/icon.png"] = resized(ring, 1024)
    expected["crates/bins/desktop-shell/ui/public/favicon.png"] = resized(ring, 96)
    expected["crates/bins/desktop-shell/ui/public/onyx-ring.png"] = resized(ring, 1024)
    expected["web-ui/public/favicon.png"] = resized(ring, 96)
    expected["mobile-pwa/public/icons/icon-192.png"] = resized(ring, 192)
    expected["mobile-pwa/public/icons/icon-512.png"] = resized(ring, 512)
    expected["mobile-pwa/public/icons/icon-192-maskable.png"] = padded_maskable(ring, 192)
    expected["mobile-pwa/public/icons/icon-512-maskable.png"] = padded_maskable(ring, 512)
    expected["mobile-pwa/public/icons/apple-touch-icon.png"] = resized(ring, 180)
    densities = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
    for density, size in densities.items():
        icon = padded_maskable(ring, size)
        expected[f"mobile-android/app/src/main/res/mipmap-{density}/ic_launcher.png"] = icon
        expected[f"mobile-android/app/src/main/res/mipmap-{density}/ic_launcher_round.png"] = icon
    for relative, image in expected.items():
        actual = validate_png(relative, EXPECTED_PNGS[relative])
        assert_matches(relative, actual, image)


def main() -> int:
    for relative, dims in EXPECTED_PNGS.items():
        validate_png(relative, dims)
    if (ROOT / "assets/branding/admin-shell.png").is_file() and (ROOT / "assets/branding/onyx-ring.png").is_file():
        try:
            admin = Image.open(ROOT / "assets/branding/admin-shell.png").convert("RGBA")
            ring = Image.open(ROOT / "assets/branding/onyx-ring.png").convert("RGBA")
            if admin.size == ring.size and ImageChops.difference(admin, ring).getbbox() is None:
                fail("Admin Shell source is identical to the standard ONYX ring; the approved custom exception appears lost")
        except Exception as error:
            fail(f"cannot compare Admin Shell exception with standard ring ({error})")
    validate_generated_mappings()
    validate_tauri_containers()
    validate_references()
    if ERRORS:
        print("ONYX icon validation FAILED:")
        for error in ERRORS:
            print(f" - {error}")
        return 1
    print(f"ONYX icon validation PASSED: {len(CHECKED)} PNG files checked.")
    for relative, (width, height, transparency) in sorted(CHECKED.items()):
        print(f"OK {relative}: {width}x{height}, fully transparent pixels {transparency:.2%}")
    print("Validated Tauri ICO/ICNS derivatives, both Tauri configs, all HTML favicon references, PWA manifest entries, Android launcher densities, and the Staff login ring reference.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
