# ONYX platform icon assets

## Canonical transparent exports

The launcher/build pipeline creates two canonical transparent PNG exports:

- `assets/branding/onyx-ring-transparent.png` — standard ONYX ring for the Staff desktop shell, web operator, Mobile Observer PWA, and Android launcher.
- `assets/branding/admin-shell-transparent.png` — the existing custom Admin Shell icon, preserved as an explicit exception.

The repository's original `assets/branding/onyx-ring.png` and `assets/branding/admin-shell.png` masters were flattened against black (the master alpha channel is opaque). They remain input masters, not direct application-icon assets. The generator exports the existing visible artwork into transparent canonical PNGs using a black-matte removal step only when all four image corners are near-black. It unpremultiplies the RGB values to retain the existing edge color/coverage, rejects unexpected backgrounds instead of guessing, and does not redraw or redesign either icon.

Platform-specific PNGs are generated from those transparent exports using premultiplied-alpha Lanczos resizing. Maskable PWA and Android icons retain transparent padding around the logo. Tauri receives PNG plus required ICO/ICNS packaging derivatives generated from the same approved PNG source.

## Audit/validation

Run from the repository root:

    python3 -m pip install pillow
    python3 scripts/branding/generate-native-icons.py
    python3 scripts/branding/validate-icons.py

The validator checks actual PNG encoding, dimensions, real alpha transparency (including all outer corners and the standard ring's hollow center), visible artwork/clipping, source-to-output pixel correspondence, Tauri ICO/ICNS entry sizes and transparency, HTML favicon paths, PWA manifest paths/sizes/purposes, Android launcher resources at all densities, and the Staff login's ring-image reference.

The Node entry point `scripts/branding/run-icon-generator.mjs` runs both scripts. Frontend predev/prebuild hooks, Android Gradle preBuild, CI, and release workflows call it (or its equivalent) before consuming icons.

## Discovered targets

| Target | Configuration / consumer reference | Transparent source |
| --- | --- | --- |
| Admin Shell (Tauri desktop) | `crates/bins/admin-shell/tauri.conf.json`; `ui/public/favicon.png` | `assets/branding/admin-shell-transparent.png` |
| Staff desktop shell (Tauri) | `crates/bins/desktop-shell/tauri.conf.json`; `ui/public/favicon.png`; `ui/public/onyx-ring.png` | `assets/branding/onyx-ring-transparent.png` |
| Web operator application | `web-ui/index.html`; `web-ui/public/favicon.png` | `assets/branding/onyx-ring-transparent.png` |
| Mobile Observer PWA | `mobile-pwa/index.html`; `mobile-pwa/public/manifest.webmanifest`; `mobile-pwa/public/icons/` | `assets/branding/onyx-ring-transparent.png` |
| Android native app | `mobile-android/app/src/main/AndroidManifest.xml`; `res/mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/` | `assets/branding/onyx-ring-transparent.png` |

Both Tauri configurations target all and provide PNG, ICO, and ICNS bundle inputs, covering the applicable Linux/Windows/macOS package formats. Those OS package formats are outputs of the two discovered desktop shells, not separate source applications. No standalone iOS target or additional independent desktop shell was found. The Staff UI's horizontal `onyx-logo-horizontal.png` is a separate in-app wordmark lockup used in navigation; it is retained. Functional UI icons (Lucide and Android Material icons) are not ONYX brand assets and remain untouched.
