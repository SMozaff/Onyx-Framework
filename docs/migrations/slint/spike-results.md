# Slint feasibility spike results (brief B-13)

**Source:** `docs/migrations/slint/spike-handover.md` (spikes U-01, U-02, U-03, U-10).
**Code:** `spikes/slint/` (Slint 1.18.1, Rust, standalone crate).
**Run date:** 2026-11-01. **Environment:** Linux cloud container (Ubuntu 24.04 base, no physical display, no IME, no screen reader, no Windows host).
**Status:** Gate 0b is not passed by this document. Owner sign-off is still required.

## Summary

**Passed (automated, Linux only):**
- U-01.1 sortable table: header clicks sort ascending, then descending; score sort is ordered.
- U-01.2 paged table: next and previous paging, "Page n of N", Previous and Next enable state.
- U-03 font choice: Vazirmatn 33.0.3 (OFL-1.1). Recorded with checksums. Glyph coverage for Latin, Persian, digits, joining, harakat and bidi marks is complete.
- U-10 Linux GNOME read and toggle (isolated GSettings keyfile backend), and the unknown or unreadable fallback to reduced motion.

**Failed:**
- U-10 Windows: the OS reduce-motion setting is not read. The Windows reader is a stub (`read_windows_animations()` returns `None`), so Windows always takes the fallback. Limit: this code cannot be compiled or run in this environment. Proposed fix (for review): read `SPI_GETCLIENTAREAANIMATION` with `SystemParametersInfoW`, in the platform layer (B-12).

**Partial (automated evidence, but a visible or OS-level part is not verified):**
- U-01.3 multiline: Enter inserts a newline (value level). Visual wrapping is not checked.
- U-01.5 mixed direction: Persian and English values round-trip. The screenshot shows the full stop of a mixed line at the visual left end of the Persian run. Cursor movement, selection, and copy-paste are not tested.
- U-02.1 labels and roles: correct in the Slint element tree. Not checked through AT-SPI or UI Automation.
- U-03 shaping and glyph loading (Linux): the font loads and renders under Xvfb. Shaping is checked only by agent screenshot.
- U-10 KDE: the `kdeglobals` parser is tested with a file. A real KDE session is not tested.
- U-10 toggle: the setting is read at startup only. A live change while the app runs is not handled.

**Needs manual testing by a person (`pending-manual`):**
- All Windows rows (U-01, U-02, U-03, U-10). No Windows host here.
- U-01.4 Persian IME (Windows IME and Linux IBus or Fcitx). Composition is not tested.
- U-02.2 keyboard Tab order, focus visibility, Enter and Escape on controls.
- U-02.3 screen reader output: NVDA on Windows, Orca on Linux. Neither is installed here. The AT-SPI registry is not installed either.
- U-02.4 focus indicator visible in light and dark themes.
- U-03.2 and U-03.3 visual shaping check on both OS (the captured screenshot needs a human check).

**Open questions (no product decisions were made here):** see the list at the end.

## Results

Result values: `pass`, `fail`, `partial`, `pending-manual`. Evidence paths are relative to the repository root.

| Spike | Item | OS | Result | Evidence | Notes |
|---|---|---|---|---|---|
| U-01 | 1. Sortable table | Linux | pass | `spikes/slint/tests/ui_behaviour.rs` (header clicks); `spikes/slint/src/logic.rs` unit tests; `spikes/slint/evidence/cargo-test-linux.txt` | Headless Slint testing backend with mock clicks. No pointer session. A sort-direction indicator is text only (`Sorted by ...`). |
| U-01 | 1. Sortable table | Windows | pending-manual | none | No Windows host. Same `.slint` code is expected to behave the same. Not verified. |
| U-01 | 2. Paged table | Linux | pass | `tests/ui_behaviour.rs` (page 1 to 3 contents, disabled Previous and Next at the ends); `logic.rs` page tests | Page size 4 over 10 sample rows. |
| U-01 | 2. Paged table | Windows | pending-manual | none | No Windows host. |
| U-01 | 3. Multiline text | Linux | partial | `tests/ui_behaviour.rs` (typing "ab", Enter, "cd" gives "ab\ncd"); `ui/app.slint` sets `wrap: word-wrap` | Enter works at value level. Wrapping of a long line on screen is not checked. |
| U-01 | 3. Multiline text | Windows | pending-manual | none | No Windows host. |
| U-01 | 4. Persian IME | Linux | pending-manual | `tests/ui_behaviour.rs` shows simulated keystrokes ("سلام دنیا") reach the binding. This is not IME. | No IME in the container. Composition and text loss need a person with an IBus or Fcitx Persian input method. |
| U-01 | 4. Persian IME | Windows | pending-manual | none | Needs a person with a Windows Persian IME. |
| U-01 | 5. Bidi text | Linux | partial | `tests/ui_behaviour.rs` (value "Hello سلام world" round-trips); `evidence/u03-u01-screenshot-linux-xvfb.png` | Screenshot: the Persian placeholder is left-aligned. In the line "Persian: سلام، این یک متن آزمایشی فارسی است." the full stop sits at the visual left end of the Persian run (agent observation; a person should confirm). Cursor across the boundary, selection, and copy-paste direction not tested. See OQ-3. |
| U-01 | 5. Bidi text | Windows | pending-manual | none | Needs a person on Windows. |
| U-02 | 1. Labels | Linux | partial | `tests/ui_behaviour.rs` (6 buttons with role Button; 3 text inputs with role TextInput; all with accessible labels) | Checked in the Slint element tree only. Not checked through AT-SPI, because no AT-SPI registry is installed here. |
| U-02 | 1. Labels | Windows | pending-manual | none | UI Automation not checked. |
| U-02 | 2. Keyboard (Tab order, focus, Enter, Escape) | Linux | pending-manual | none | Not automated in this run. Enter in the multiline field is covered by U-01.3 only. |
| U-02 | 2. Keyboard | Windows | pending-manual | none | Needs a person. |
| U-02 | 3. Screen reader output (button, input, table header) | Linux (Orca) | pending-manual | none | Orca and the AT-SPI registry are not installed. |
| U-02 | 3. Screen reader output | Windows (NVDA) | pending-manual | none | NVDA not available here. |
| U-02 | 4. Focus indicator, light and dark | Linux | pending-manual | `tests/ui_behaviour.rs` (theme toggle flips `Theme.dark`) | Theme switch works. The focus ring is the default std-widgets style; the custom ring from C-04 is not built. Visual check needed. See OQ-4. |
| U-02 | 4. Focus indicator, light and dark | Windows | pending-manual | none | Needs a person. |
| U-03 | 1. Font choice, version, license | Both (file) | pass | `spikes/slint/fonts/Vazirmatn-Regular.ttf` (sha256 `b69fd4c680b8f3f225feabcc655a2c585d97627b8f5f5c0f9985e894069f3a56`); `spikes/slint/fonts/OFL.txt` (sha256 `17e355067c8284f47743a1ee3b1ef7ff684ff0601eda357f9353b10b3016ab31`); decision log "Gate 0b coordination and font (2026-10-29)" | Vazirmatn 33.0.3, SIL OFL 1.1, from the npm package `vazirmatn` (upstream release). The owner confirmed the font and license on 2026-10-29. Font embedded at compile time through `import` in `ui/app.slint`. |
| U-03 | 2. Samples render with correct shaping | Linux | partial | `spikes/slint/evidence/font-coverage-linux.txt` (all required code points present); `spikes/slint/evidence/u03-u01-screenshot-linux-xvfb.png` (sample block at "U-03 font samples") | Coverage is automated. The screenshot shows joined Persian, ZWNJ, harakat, and Latin and Persian digits. Shaping was judged from the agent's capture, not by a person. Full-stop placement issue, see U-01.5. |
| U-03 | 2. Samples render with correct shaping | Windows | pending-manual | none | Needs a person to view the sample block on Windows. |
| U-03 | 3. Glyphs load on both OS | Linux | partial | `evidence/smoke-linux.txt` (window starts and renders under Xvfb); `spikes/slint/scripts/smoke-screenshot.sh` | Loads with compile-time embedding. The app log shows only an xdg-settings D-Bus notice (no session bus in the container). |
| U-03 | 3. Glyphs load on both OS | Windows | pending-manual | none | Needs a person. |
| U-10 | 1. Read OS setting | Linux (GNOME) | pass | `evidence/motion-check-linux.txt` (cases 1 and 2, `gsettings` `enable-animations` read through an isolated keyfile backend) | Real GNOME session (dconf) not tested. The API is the same `gsettings` call. |
| U-10 | 1. Read OS setting | Linux (KDE) | partial | `evidence/motion-check-linux.txt` (cases 4 and 5, `kdeglobals` `[KDE] AnimationDurationFactor`) | Parser tested with files. No real KDE session. `kreadconfig` is not used. |
| U-10 | 1. Read OS setting | Windows | fail | `spikes/slint/src/logic.rs` `read_windows_animations()` returns `None` | Not implemented. Cannot be compiled or run here. Proposed for review: `SystemParametersInfoW` with `SPI_GETCLIENTAREAANIMATION` in the platform layer (B-12). |
| U-10 | 2. Toggle, flag changes in Slint | Linux | partial | `evidence/motion-check-linux.txt` (cases 1 and 2: setting off gives `Motion.reduced = true`; on gives `false`) | Read at startup only. A change while the app runs is not handled. See OQ-1. |
| U-10 | 2. Toggle, flag changes in Slint | Windows | fail | as above | Same cause as item 1. |
| U-10 | 3. Unknown or unreadable falls back to reduced | Linux | pass | `evidence/motion-check-linux.txt` (cases 3 and 5: no `gsettings` on PATH; corrupt KDE value; both give `reduced`); `logic.rs` unit tests | Applies decision U-10 (2026-10-31). |
| U-10 | 3. Unknown or unreadable falls back to reduced | Windows | pending-manual | none | The logic is shared, but the Windows reader does not exist yet, so the fallback is the only Windows path. Test again after the reader is written. |

## How to reproduce

```
cd spikes/slint
cargo test                                   # unit tests and headless UI checks (no display needed)
python3 scripts/font-coverage.py             # U-03 glyph coverage (needs fonttools)
cargo build
BIN=target/debug/onyx-slint-spike xvfb-run -a scripts/motion-check.sh        # U-10 Linux
BIN=target/debug/onyx-slint-spike OUT=shot.png xvfb-run -a -s "-screen 0 1280x1800x24" scripts/smoke-screenshot.sh
```

The repository's `rust-toolchain.toml` pins Rust 1.97.1. In this container, rustup could not download that toolchain (`static.rust-lang.org` returned 403 through the egress proxy). The spike was built with the installed stable toolchain (`RUSTUP_TOOLCHAIN=stable`, cargo 1.97.0). Rebuild with the pinned toolchain before relying on these results.

## Open questions

These need an owner or design decision. None was made here.

- **OQ-1 (owner, affects C-05 and B-12):** Should the app react to a change of the OS motion setting while it runs, or read it at startup only? The spike reads it at startup only.
- **OQ-2 (owner, Gate 0b):** If the Windows reader is not written and tested before Gate 0b, is "reduced motion on Windows" an accepted temporary state? Or does it block Gate 0b?
- **OQ-3 (D3 and D4 with the owner, C-06):** Which alignment and direction rule applies to a Persian placeholder and to mixed-direction lines? The spike shows the Persian placeholder left-aligned, and the full stop of a mixed line at the visual left end. Decision on direction per field, and on how punctuation is handled, is open.
- **OQ-4 (D3 with the owner, C-04):** Keep the std-widgets focus style, or build the custom focus ring that C-04 implies? Focus visibility in both themes is pending manual review.
- **OQ-5 (owner):** Who runs the pending-manual checks (Windows host, Persian IME, NVDA, Orca, light and dark focus), and on which dates?

## Limits of this run

- No display, screen reader, IME, or Windows host. Screenshots were captured under Xvfb and inspected by the agent. Visual results are not human-confirmed.
- Disk space in the container was exhausted during the build. To free space, the spike removed the regenerable `~/.cargo/registry/src` extraction and stale spike build outputs. The repository `target/` directory was not touched.
- The Slint testing backend (`i-slint-backend-testing 1.18.1`, pinned) drives clicks and key events without a real pointer or IME. It does not test real focus traversal.
