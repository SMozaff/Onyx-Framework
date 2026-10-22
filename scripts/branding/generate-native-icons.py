#!/usr/bin/env python3
"""Generate all ONYX launcher assets from the committed transparent PNG brand sources."""
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
BRANDS = {
    "admin-shell": ROOT / "crates/bins/admin-shell",
    "desktop-shell": ROOT / "crates/bins/desktop-shell",
}
RESAMPLING = Image.Resampling.LANCZOS
TRANSPARENT_PIXEL_THRESHOLD = 0.05


def load_square(path: Path) -> Image.Image:
    with Image.open(path) as original:
        if original.format != "PNG":
            raise ValueError(f"Brand source must be an actual PNG: {path} ({original.format})")
        if original.mode not in ("RGBA", "LA") and "transparency" not in original.info:
            raise ValueError(f"Brand source has no alpha/transparency channel: {path}")
        image = original.convert("RGBA")
    if image.width != image.height:
        raise ValueError(f"Brand source must be square: {path} ({image.size})")
    alpha = image.getchannel("A")
    transparent_fraction = sum(alpha.histogram()[:1]) / (image.width * image.height)
    if alpha.getextrema()[0] != 0 or transparent_fraction < TRANSPARENT_PIXEL_THRESHOLD:
        raise ValueError(
            f"Brand source has an opaque/baked background or insufficient transparency: "
            f"{path} ({transparent_fraction:.2%} fully transparent pixels)"
        )
    corners = (
        alpha.getpixel((0, 0)),
        alpha.getpixel((image.width - 1, 0)),
        alpha.getpixel((0, image.height - 1)),
        alpha.getpixel((image.width - 1, image.height - 1)),
    )
    if any(value != 0 for value in corners):
        raise ValueError(f"Brand source corners must be fully transparent: {path} ({corners})")
    if alpha.getbbox() is None:
        raise ValueError(f"Brand source is blank: {path}")
    return image


def save_png(image: Image.Image, path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    image.save(path, format="PNG", optimize=True)


def resized(image: Image.Image, size: int) -> Image.Image:
    """Resize premultiplied RGBA to avoid color fringes around transparent edges."""
    if image.size == (size, size):
        return image.copy()
    return image.convert("RGBa").resize((size, size), RESAMPLING).convert("RGBA")


def padded_maskable(image: Image.Image, size: int) -> Image.Image:
    """Fit the whole ring inside the platform's central 80% safe zone, transparently."""
    canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
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
    save_png(resized(image, 96), crate / "ui/public/favicon.png")
    print(f"Generated transparent PNG/ICO/ICNS Tauri launcher assets for {name}.")


def generate_mobile_and_web_icons(image: Image.Image, desktop_crate: Path) -> None:
    save_png(resized(image, 96), ROOT / "web-ui/public/favicon.png")
    save_png(resized(image, 1024), desktop_crate / "ui/public/onyx-ring.png")
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
    print("Generated transparent PNG web, PWA, Apple touch, and Android launcher assets.")


def main() -> None:
    images: dict[str, Image.Image] = {}
    for name in ("admin-shell", "desktop-shell"):
        source_name = "admin-shell.png" if name == "admin-shell" else "onyx-ring.png"
        images[name] = load_square(ROOT / "assets/branding" / source_name)
    for name, crate in BRANDS.items():
        generate_tauri_icons(name, crate, images[name])
    generate_mobile_and_web_icons(images["desktop-shell"], BRANDS["desktop-shell"])


if __name__ == "__main__":
    main()
