#!/usr/bin/env python3
"""Generate all ONYX launcher assets from the committed PNG brand sources."""
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
BRANDS = {
    "admin-shell": ROOT / "crates/bins/admin-shell",
    "desktop-shell": ROOT / "crates/bins/desktop-shell",
}
RESAMPLING = Image.Resampling.LANCZOS


def load_square(path: Path) -> Image.Image:
    image = Image.open(path).convert("RGBA")
    if image.width != image.height:
        raise ValueError(f"Brand source must be square: {path} ({image.size})")
    return image


def save_png(image: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, format="PNG", optimize=True)


def resized(image: Image.Image, size: int) -> Image.Image:
    return image.resize((size, size), RESAMPLING)


def padded_maskable(image: Image.Image, size: int) -> Image.Image:
    """Keep the entire ring inside the platform's central 80% safe zone."""
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 255))
    inner_size = round(size * 0.8)
    inner = resized(image, inner_size)
    canvas.alpha_composite(inner, ((size - inner_size) // 2, (size - inner_size) // 2))
    return canvas


def generate_tauri_icons(name: str, crate: Path, image: Image.Image) -> None:
    out = crate / "icons"
    out.mkdir(parents=True, exist_ok=True)
    source = resized(image, 1024)
    save_png(source, out / "icon.png")
    source.save(
        out / "icon.ico",
        format="ICO",
        sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
    )
    source.save(out / "icon.icns", format="ICNS")
    favicon = crate / "ui/public/favicon.png"
    save_png(resized(image, 96), favicon)
    print(f"Generated PNG/ICO/ICNS Tauri launcher assets for {name}.")


def generate_mobile_and_web_icons(image: Image.Image) -> None:
    save_png(resized(image, 96), ROOT / "web-ui/public/favicon.png")
    pwa = ROOT / "mobile-pwa/public/icons"
    for size in (192, 512):
        save_png(resized(image, size), pwa / f"icon-{size}.png")
        save_png(padded_maskable(image, size), pwa / f"icon-{size}-maskable.png")
    save_png(resized(image, 180), pwa / "apple-touch-icon.png")

    android = ROOT / "mobile-android/app/src/main/res"
    densities = {"mdpi": 48, "hdpi": 72, "xhdpi": 96, "xxhdpi": 144, "xxxhdpi": 192}
    for density, size in densities.items():
        resource_dir = android / f"mipmap-{density}"
        icon = padded_maskable(image, size)
        save_png(icon, resource_dir / "ic_launcher.png")
        save_png(icon, resource_dir / "ic_launcher_round.png")
    print("Generated PNG web, PWA, Apple touch, and Android launcher assets.")


for name, crate in BRANDS.items():
    source_name = "admin-shell.png" if name == "admin-shell" else "onyx-ring.png"
    image = load_square(ROOT / "assets/branding" / source_name)
    generate_tauri_icons(name, crate, image)
    if name == "desktop-shell":
        generate_mobile_and_web_icons(image)
