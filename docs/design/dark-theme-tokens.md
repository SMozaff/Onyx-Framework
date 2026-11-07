# Dark theme tokens (owner approved, 2026-11-07)

Source palette: ONYX Brand Identity / Color System.
Contrast ratios are WCAG 2.x, measured against the dark backgrounds.

## Mapping (approved)

| Role | Token | Value | Palette name | Contrast on Void/Deep/Midnight |
|---|---|---|---|---|
| App background | bg | `#010314` | ONYX Void | — |
| Panels | surface | `#020728` | ONYX Deep | — |
| Raised surfaces | surface-hover | `#061250` | ONYX Midnight | — |
| Primary text | text | `#F2F5FC` | Signal White | 18.8 / 18.1 / 15.9 |
| Secondary text | text-secondary | `#E0E5F5` | Ice | 16.3 / 15.7 / 13.8 |
| Muted text | text-dim | `#8B95A8` | (new, approved) | 6.8 / 6.6 / 5.8 |
| Interactive | accent | `#1747FF` | Field Blue | — |
| Primary signal | signal | `#0B60FA` | Signal Blue | (shapes/large only) |
| Live status | live | `#39D9FF` | Cyan Edge | 12.2 / 11.8 / 10.4 |

## Rules

- Text on dark backgrounds must meet WCAG AA (4.5:1 for body text).
- Graphite `#59657A` is not used for text on dark backgrounds (2.95–3.48:1, fails AA).
- Signal Blue `#0B60FA` is not used for text on dark backgrounds (3.4–4.0:1).

## Decided by the team (2026-11-07, owner delegated)

Status and border colours. Text contrast is measured against Void/Deep/Midnight.

| Token | Value | Reason | Contrast (Void/Deep/Midnight) |
|---|---|---|---|
| border | `#33466E` | Strongest tone that stays subtle. Non-text boundary; no palette colour is both subtle and 3:1. | 2.2 / 2.1 / 1.9 (non-text) |
| status-draft | `#8B95A8` | Same as muted text; neutral state. | 6.8 / 6.6 / 5.8 |
| status-active | `#39D9FF` | Cyan Edge (palette, live signal). | 12.2 / 11.8 / 10.4 |
| status-review | `#A77BFF` | Violet family lightened from Ion Violet to pass 4.5:1 on all dark backgrounds. | 6.8 / 6.5 / 5.7 |
| status-approved | `#3DDC97` | New green (not in palette). Chosen for clear meaning and contrast. | 11.6 / 11.2 / 9.8 |
| status-blocked | `#FF6B6B` | New red (not in palette). Chosen for clear meaning and contrast. | 7.4 / 7.1 / 6.3 |
| status-closed | `#8B95A8` | Inactive state; same as muted text. | 6.8 / 6.6 / 5.8 |

Note: green and red are new colours not in the brand palette. Owner may replace them with palette values later.

## Status (2026-11-07)

- **Wired** in the admin UI. A Theme switch (Light/Dark) sits in the top bar and remembers the choice on the device. Default is Light.
- **Hard-coded light colours removed** from admin components and `index.css` surfaces (`bg-white`, `bg-slate-*`, `bg-sky-50`, auth card and page background, workspace header). Status notices, the connection banner, and sidebar keep their own colours.
- **Light-mode shift:** `bg-slate-50/100` and `bg-sky-50` became `#e8f0f8` (Onyx surface-hover). A small visual change in Light mode.
- **Checked:** the login page renders in both themes (screenshots). Admin tests pass (31/31) and the build passes.
- **Not checked:** the signed-in layout in a browser, which needs a session.
- **Pre-existing, not changed:** on the login card the "Language" label overlaps the "ONYX Admin" title. It is in both themes.
- **Translations:** the Theme labels in Persian (قالب، روشن، تیره) were written by the team and should be reviewed by a Persian speaker.
- **Sync:** running `scripts/sync-i18n.mjs` also regenerated the dictionaries for desktop-shell, mobile-pwa, and web-ui. Only the new `theme.*` keys changed there.
