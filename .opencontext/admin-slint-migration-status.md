---
status: active
---
# Admin/Desktop Slint migration status (updated 2026-11-25)

## Verified state
- `admin-slint`: S5/S6 complete (R-20..R-28), parity matrix at `docs/migrations/slint/admin-parity-matrix.md`, S9 PROPOSED drafts in `docs/migrations/slint/gate-0b/`. Admin CI: only pre-existing rust-advisories known red.
- `desktop-slint`: S7 complete (R-40/R-41) — 3-field org login, registry lookup, per-server session, forced password change, startup restore, tests. Dedicated Desktop Slint workflow green on Ubuntu + Windows (run 45679850793: fmt, clippy, tests, release build).
- All committed and pushed to `main` (latest `11973395`). Working tree clean.
- An unpushed local commit that removed ~24k lines of tests was dropped on owner instruction; test suites and CI configs are intact.

## Still blocked
- Gate 0b BLOCKED, Contract Freeze v1 unsigned; Gate 15 not reached. Tauri/React admin+desktop shells remain shipped (R-72 removal only after owner approval).
- CI `check` job fails workspace clippy on pre-existing `rfd` feature conflict (`gtk3` + `xdg-portal`); `merge-gate` consequent; `rust-advisories` pre-existing (D-11).
- No Worker deploy/live GUI run (R-10/R-30); Farsi review pending; Q-14/D-01/D-03/D-16 owner decisions open.

## Rules
- No local builds/tests; verification via GitHub Actions only. Push via SSH; `gh` needs `env -u GH_TOKEN`.
