#!/usr/bin/env python3
"""Generate transparent native Tauri icon files from the committed SVG brand sources."""
from pathlib import Path
import cairosvg
from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
BRANDS = {
    "admin-shell": ROOT / "crates/bins/admin-shell",
    "desktop-shell": ROOT / "crates/bins/desktop-shell",
}
for name, crate in BRANDS.items():
    source = ROOT / "assets/branding" / ("admin-shell.svg" if name == "admin-shell" else "onyx-ring.svg")
    out = crate / "icons"
    out.mkdir(parents=True, exist_ok=True)
    png = out / "icon.png"
    cairosvg.svg2png(url=str(source), write_to=str(png), output_width=1024, output_height=1024)
    image = Image.open(png).convert("RGBA")
    image.save(out / "icon.ico", format="ICO", sizes=[(16,16),(24,24),(32,32),(48,48),(64,64),(128,128),(256,256)])
    image.save(out / "icon.icns", format="ICNS")
    print(f"Generated transparent icons for {name}: {png}, icon.ico, icon.icns")
