# Slint feasibility spikes: handover

**Status:** Ready for an agent. Owner: D13 (Verification & CI). Source: `ONYX_Slint_Migration_BOUND_Plan_Revised.md`, brief B-13 and section 1.2.
**Blocks:** Gate 0b (Contract Freeze v1). Nothing in Wave 1 starts until these pass or have a recorded fallback.

## Rules for the agent

- Work only in `spikes/slint/` (create it). Do not edit `crates/bins/admin-shell`, `crates/bins/desktop-shell`, the Worker, the Axum API, or the database schema. Scope boundary: plan section 12.
- Do not store, log, or prefill passwords. Test login is not part of these spikes.
- Do not decide product questions. If a result needs a product choice, record it as an open question.
- Record every result in `docs/migrations/slint/spike-results.md`, with pass, fail, or partial, the evidence, and the OS.
- Do not claim a manual test passed unless a person ran it. Mark it `pending-manual` instead.
- Keep the spike code small. It exists only to answer the questions below.

## Spike U-01: widget feasibility

Check each item on Linux and Windows. Prototype five components with test data.

1. Sortable table: click a header, rows reorder.
2. Paged table: next and previous page, page count shown.
3. Multiline text area: Enter inserts a newline, text wraps.
4. Persian IME input: type Persian with an IME, text appears correctly, composition is not lost. Manual test on Windows and Linux.
5. Bidirectional text: Persian and English mixed in one field, cursor moves correctly across the boundary, selection and copy-paste keep direction.

**Pass:** all five work on both OS. **Fail:** list each failing item, its limit, and any workaround or fallback. Do not invent a fallback; propose one and mark it for review.

## Spike U-02: accessibility

Test on Windows (UI Automation) and Linux (AT-SPI).

1. Labels: each input has an accessible name.
2. Keyboard: Tab order is correct, focus is visible, Enter and Escape work.
3. Screen reader output: read a button, an input, and a table header with NVDA on Windows and Orca on Linux. Manual.
4. Focus indicator visible in light and dark themes.

Record pass or fail per feature per OS. Mark items that need a screen reader as `pending-manual` if no person runs them.

## Spike U-03: font coverage

1. Pick a bundled font with Latin and Persian (Arabic-script) coverage. Record the font name, version, and license. The owner has already confirmed the font choice and license; check the decision log before choosing.
2. Render sample Latin a-z, Persian text with correct joining and shaping, combining marks, and Latin and Persian digits.
3. Check the glyphs load on Windows and Linux. Manual visual check.

**Pass:** all samples render with correct shaping. Record file paths and checksums.

## Spike U-10: reduce-motion setting

1. Read the OS reduce-motion setting on Windows and on Linux (GNOME or KDE).
2. Toggle the setting and confirm the app's animation flag changes.
3. Unknown or unreadable setting falls back to reduced motion (decision U-10, 2026-11-01). Test that path.

**Pass:** the setting is read on both OS and the fallback works.

## Deliverables

- `spikes/slint/` code (small, runnable).
- `docs/migrations/slint/spike-results.md` with one row per item: spike, item, OS, result (pass, fail, partial, pending-manual), evidence, and notes.
- A short summary at the top of the results file stating what passed, what failed, and what needs manual testing by a person.

## Stop conditions

- A Slint version or dependency cannot be installed: record it and stop that spike.
- A result needs a product decision: record it as an open question, then continue.
