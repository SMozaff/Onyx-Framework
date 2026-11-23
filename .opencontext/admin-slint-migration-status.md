# Admin Slint migration status (updated 2026-11-23)

## Verified state
- `crates/bins/admin-slint/` is a feasibility client with all Admin pages: sign-in, Users, Settings (mobile access + policy/rules + legal holds), Profiles, Organizations (All-Father-gated).
- S5 (R-20..R-23): code-based concurrency error mapping, structured evaluation display, per-policy pending rules with removal, legal-hold retry key retention and release-reason input.
- S6 (R-24..R-28): `docs/migrations/slint/admin-parity-matrix.md` (AD-01..AD-07, AD-I-001..AD-I-061); R-25 validation/busy bindings implemented; R-26 Users search/sort/paging determined NOT a parity gap (blocked on owner approval + U-01 evidence); R-28 shared Latin-digit helper applied to read-only hold-ID display.
- S9: `docs/migrations/slint/gate-0b/` holds PROPOSED drafts (C-01, C-03, C-09, Q-14, R-17, R-29 23 EN=FA keys, R-62 plan path). Dated amendment appended to MASTER-MIGRATION-RECORD.md + decision log. No blocker closed, no contract frozen.
- All work committed and pushed to `main` (https://github.com/SMozaff/Onyx-Framework). Latest push: `9bafe451` (rustfmt fix from CI). Working tree clean.

## Still blocked / not done
- **Gate 0b BLOCKED**, Contract Freeze v1 unsigned. No production Admin modules authorized.
- Tauri admin (`crates/bins/admin-shell`) is still the shipped client; removal is Phase 16 / R-72 after Gate 15 + owner approval.
- Worker not deployed (R-10); no live click-through (R-30).
- Remaining parity gaps: access-denied screen, remember-username, password show/hide, profile edit overlay, open-policy-by-id, Users search/sort/paging.
- Farsi review pending (D-01/D-03): several labels still English in `fa.json`.
- CI: rust-advisories failure is pre-existing (D-11); other jobs must be read from latest PR head.

## Working agreements
- Verification is via GitHub Actions only — no local builds/tests (owner instruction).
- Push uses SSH keyring because GH_TOKEN is invalid; local git config rewrites https://github.com/ to git@github.com: for this repo.
