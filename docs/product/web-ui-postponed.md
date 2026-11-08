# Web UI: postponed (owner decision, 2026-11-08)

Status: postponed. Not a priority. Do not start without owner approval.

## 1. File transfer on web (Internet only)

- **Scope:** web-ui has no Files page. Add one.
- **Route:** Internet only (100 MB per attempt). LAN and emergency P2P shown as disabled items, same as desktop.
- **Reference:** desktop `crates/bins/desktop-shell/ui/src/pages/Files.tsx` and `components/TransferRoutes.tsx`. Reuse the same i18n keys (`files.route*`).
- **Steps when resumed:**
  1. Add the route and page under web-ui routing.
  2. Wire upload and download to the same Internet API the desktop uses (see `docs/product/file-transfer.md`).
  3. Show disabled LAN and emergency items.
  4. Add tests for the 100 MB limit message and the disabled states.

## 2. Dependency blockers for web-ui

- **#424 (msw 3.0.3) and #429 (vitest 5.1.0):** both fail `npm ci` with ERESOLVE. Both need `vite >= 6`. web-ui is pinned to `vite 5.4.21`.
- **Fix when resumed:**
  1. Upgrade `vite` to 6.x in `web-ui`, and check `@vitejs/plugin-react` supports it.
  2. Re-run `npm ci`, tests, and build.
  3. Rebase or re-open #424 and #429 and merge.

## 3. Related blocker (mobile-pwa, not web-ui)

- **#423 (eslint 10.14.0):** `eslint-plugin-react-hooks@4.6.2` needs ESLint 8 or older. Upgrade the plugin to a version that supports ESLint 10, then re-open #423.

## 4. Audit note

- web-ui `npm audit` reports 13 findings (5 high) in the dev tree on the vitest branch. Check whether `main` has the same findings before fixing. Production dependencies were not the subject of these findings.

## Owner input needed when resumed

- Confirm web file-transfer size limit stays at 100 MB.
- Confirm web should show the same disabled LAN and emergency items as desktop.
