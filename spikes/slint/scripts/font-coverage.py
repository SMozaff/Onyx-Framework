#!/usr/bin/env python3
"""U-03 check: glyph coverage of the bundled font, and its name table and checksum.

Requires fontTools (pip install fonttools). Run from anywhere:
    python3 spikes/slint/scripts/font-coverage.py
Exit code 1 if any required code point is missing.
"""
import hashlib
import pathlib
import sys

from fontTools.ttLib import TTFont

FONT = pathlib.Path(__file__).resolve().parent.parent / "fonts" / "Vazirmatn-Regular.ttf"

REQUIRED = {
    "Latin a-z": range(0x61, 0x7B),
    "Latin A-Z": range(0x41, 0x5B),
    "Persian letters (peh, cheh, zheh, keheh, gaf, farsi yeh, alef madda, heh goal)": [
        0x067E, 0x0686, 0x0698, 0x06A9, 0x06AF, 0x06CC, 0x0622, 0x06C1],
    "Arabic-Indic digits 0-9": range(0x0660, 0x066A),
    "Extended Arabic-Indic (Persian) digits 0-9": range(0x06F0, 0x06FA),
    "Zero-width non-joiner and joiner": [0x200C, 0x200D],
    "Harakat (fatha, damma, kasra, shadda, sukun)": [0x064B, 0x064E, 0x064F, 0x0650, 0x0651, 0x0652],
    "Bidi marks (LRM, RLM)": [0x200E, 0x200F],
    "Persian punctuation (comma, question mark, full stop)": [0x060C, 0x061F, 0x06D4],
}


def main() -> int:
    font = TTFont(str(FONT))
    cmap = font.getBestCmap()
    name = font["name"]
    digest = hashlib.sha256(FONT.read_bytes()).hexdigest()
    print(f"file: {FONT}")
    print(f"sha256: {digest}")
    print(f"family: {name.getDebugName(1)}")
    print(f"version: {name.getDebugName(5)}")
    print(f"license: {name.getDebugName(13)[:70]}...")
    print(f"GSUB present: {'GSUB' in font}  GPOS present: {'GPOS' in font}")
    failed = False
    for label, cps in REQUIRED.items():
        missing = [f"U+{c:04X}" for c in cps if c not in cmap]
        status = "OK" if not missing else "MISSING " + ", ".join(missing)
        print(f"{status:>10}  {label}")
        failed = failed or bool(missing)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
