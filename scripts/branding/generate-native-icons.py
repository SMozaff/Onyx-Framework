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


def black_matte_to_alpha(image: Image.Image, path: Path) -> Image.Image:
    """Remove a black matte from an existing logo without redrawing its artwork.

    The input files in this repository are flattened against pure black. For such
    an image, each RGB pixel is the foreground color premultiplied by coverage.
    Recover coverage from the strongest channel and unpremultiply the foreground.
    A small near-black cutoff removes compression/noise from the old matte.
    """
    rgba = image.convert("RGBA")
    corners = (
        rgba.getpixel((0, 0))[:3],
        rgba.getpixel((rgba.width - 1, 0))[:3],
        rgba.getpixel((0, rgba.height - 1))[:3],
        rgba.getpixel((rgba.width - 1, rgba.height - 1))[:3],
    )
    if any(max(pixel) > 12 for pixel in corners):
        raise ValueError(
            f"Cannot safely remove a black matte from {path}: all four corners must be near black; "
            f"corner RGB values were {corners}"
        )

    source = rgba.load()
    for y in range(rgba.height):
        for x in range(rgba.width):
            red, green, blue, _old_alpha = source[x, y]
            alpha = max(red, green, blue)
            if alpha <= 12:
                source[x, y] = (0, 0, 0, 0)
            else:
                source[x, y] = (
                    min(255, round(red * 255 / alpha)),
                    min(255, round(green * 255 / alpha)),
                    min(255, round(blue * 255 / alpha)),
                    alpha,
                )
    return rgba


def load_square(path: Path, *, allow_black_matte: bool = False) -> Image.Image:
    with Image.open(path) as original:
        if original.format != "PNG":
            raise ValueError(f"Brand source must be an actual PNG: {path} ({original.format})")
        image = original.convert("RGBA")
    if image.width != image.height:
        raise ValueError(f"Brand source must be square: {path} ({image.size})")

    def is_transparent_logo(candidate: Image.Image) -> bool:
        alpha = candidate.getchannel("A")
        transparent_fraction = alpha.histogram()[0] / (candidate.width * candidate.height)
        corners = (
            alpha.getpixel((0, 0)),
            alpha.getpixel((candidate.width - 1, 0)),
            alpha.getpixel((0, candidate.height - 1)),
            alpha.getpixel((candidate.width - 1, candidate.height - 1)),
        )
        return (
            alpha.getextrema()[0] == 0
            and transparent_fraction >= TRANSPARENT_PIXEL_THRESHOLD
            and all(value == 0 for value in corners)
            and alpha.getbbox() is not None
        )

    if not is_transparent_logo(image):
        if not allow_black_matte:
            alpha = image.getchannel("A")
            ratio = alpha.histogram()[0] / (image.width * image.height)
            raise ValueError(
                f"Brand export is not genuinely transparent: {path} "
                f"({ratio:.2%} fully transparent pixels)"
            )
        image = black_matte_to_alpha(image, path)
        if not is_transparent_logo(image):
            raise ValueError(f"Black-matte removal did not produce a valid transparent logo: {path}")
    return image


def prepare_transparent_source(input_path: Path, output_path: Path) -> Image.Image:
    """Export the designated brand artwork as a transparent canonical PNG.

    Already-transparent sources are preserved. Legacy files flattened on black
    are de-matted only when all four outer corners are near black; other
    backgrounds fail closed rather than being guessed.
    """
    image = load_square(input_path, allow_black_matte=True)
    save_png(image, output_path)
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
    branding = ROOT / "assets/branding"
    images = {
        "admin-shell": prepare_transparent_source(
            branding / "admin-shell.png", branding / "admin-shell-transparent.png"
        ),
        "desktop-shell": prepare_transparent_source(
            branding / "onyx-ring.png", branding / "onyx-ring-transparent.png"
        ),
    }
    for name, crate in BRANDS.items():
        generate_tauri_icons(name, crate, images[name])
    generate_mobile_and_web_icons(images["desktop-shell"], BRANDS["desktop-shell"])


if __name__ == "__main__":
    main()
