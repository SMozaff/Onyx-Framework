# ONYX platform icon assets

The canonical launcher/icon sources are:

- assets/branding/onyx-ring.png — standard ONYX ring used by the Staff desktop shell, web operator, Mobile Observer PWA, and Android launcher.
- assets/branding/admin-shell.png — preserved custom Admin Shell icon; this is an intentional exception and must not be replaced with the ring.

scripts/branding/generate-native-icons.py derives platform assets with alpha-aware Lanczos resizing. Transparent padding is preserved for maskable PWA and Android resources. It also emits Tauri's required ICO and ICNS packaging derivatives. Keep these non-PNG containers as derivatives of the approved PNG source; canonical assets remain PNG.

## Audit/validation

Run from the repository root:

    python3 -m pip install pillow
    python3 scripts/branding/generate-native-icons.py
    python3 scripts/branding/validate-icons.py

The validator checks PNG encoding, dimensions, visible artwork, real alpha transparency (including fully transparent exterior pixels and image corners), source-to-output pixel correspondence, Tauri ICO/ICNS sizes and transparency, HTML favicon links, PWA manifest paths/sizes/purposes, Android launcher resources at all configured densities, and the Staff login's ring-image reference. CI runs these checks before the relevant frontend/native builds.

## Discovered targets

| Target | Configured icon reference | Brand source |
| --- | --- | --- |
| Admin Shell (Tauri desktop) | crates/bins/admin-shell/tauri.conf.json; ui/public/favicon.png | assets/branding/admin-shell.png custom exception |
| Staff desktop shell (Tauri) | crates/bins/desktop-shell/tauri.conf.json; ui/public/favicon.png; ui/public/onyx-ring.png | assets/branding/onyx-ring.png |
| Web operator application | web-ui/index.html; web-ui/public/favicon.png | assets/branding/onyx-ring.png |
| Mobile Observer PWA | mobile-pwa/index.html; mobile-pwa/public/manifest.webmanifest; mobile-pwa/public/icons/ | assets/branding/onyx-ring.png |
| Android native app | mobile-android/app/src/main/AndroidManifest.xml; res/mipmap-{mdpi,hdpi,xhdpi,xxhdpi,xxxhdpi}/ | assets/branding/onyx-ring.png |

The Tauri desktop configurations target all and provide PNG, ICO, and ICNS derivatives. That covers their applicable Linux/Windows/macOS bundle icon formats; these are package formats of the two desktop shells, not separate source applications. No standalone iOS target, additional launcher shell, or independent Windows/Linux/macOS application was discovered in the repository tree. The Staff UI's horizontal onyx-logo-horizontal.png is a distinct in-app wordmark lockup, not an application icon, and is retained for the navigation header. Native/action icons (Lucide and Android Material Icons) are functional UI assets and are outside the ONYX brand-icon migration.
