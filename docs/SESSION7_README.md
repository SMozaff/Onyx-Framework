# ONYX Session 7 — `result_large_err` Resolution + Full-Workspace Clippy Pass

Session 7 picked up the single open, unresolved item flagged at the end of
Session 6 (`AUDIT_REGISTER.md` §10: how to handle the `clippy::result_large_err`
warnings on `ApiError`), resolved it per an explicit owner decision, verified
the fix with a real toolchain install, and then — per further owner
instruction — widened to a full-workspace `cargo clippy --workspace -- -D
warnings` pass, fixing every issue that surfaced.

This session installed Rust 1.97.1 (matching `rust-toolchain.toml`) into the
working sandbox via `rustup`, since no toolchain was pre-installed. All
verification below was actually executed, not inferred.

## Decision: `result_large_err`

**Owner choice: `#[allow(clippy::result_large_err)]`**, not boxing and not
shrinking `ApiErrorBody`. Rationale, scope (11 call sites, all in
`routes/{admin,command,events,mod}.rs`), and the reasoning against the other
two options are recorded as a doc comment at
`crates/bins/api-server/src/lib.rs` (crate-level `#![allow(...)]`, applied
once, not per-function) and in full in `AUDIT_REGISTER.md` §10's resolution
addendum.

`main.rs` was checked and confirmed not to touch `ApiError` directly, so no
separate annotation was needed for the bin target.

## Files changed and why

| File | Change | Why |
|---|---|---|
| `crates/bins/api-server/src/lib.rs` | Added crate-level `#![allow(clippy::result_large_err)]` with rationale doc comment | Resolves the Session 6 open question (§10) |
| `crates/bins/api-server/src/query_handler.rs` | Collapsed a nested `if` into a match guard on the `"approval"` arm | Pre-existing `collapsible_match`, flagged in Session 6 §9 as deferred, fixed on owner instruction this session |
| `crates/bins/api-server/src/routes/events.rs` | Removed a redundant `.into()` on `value.to_string()` | Pre-existing `useless_conversion`, same as above |
| `crates/bins/api-server/src/routes/mod.rs` | Factored the 7-element storage-backend tuple type in `ApiState::new` into a named `StorageBackendHandles` type alias | Pre-existing `type_complexity`, same as above |
| `crates/bins/worker/src/scheduler_loop.rs` | Removed a redundant `.into_iter()` on a `Vec` passed to an `IntoIterator`-bound parameter | New finding, surfaced only once the check widened past `api-server` to the full workspace; not previously in the audit register |
| `crates/mobile-core/src/ffi_mobile.rs` | Added `# Safety` doc sections to 4 `unsafe extern "C"` FFI functions (`mobile_core_list_aggregates`, `mobile_core_get_sync_status`, `mobile_core_list_conflicts`, `mobile_core_resolve_conflict`) | New finding (`missing_safety_doc`), same as above. Wording follows the exact convention already established by sibling functions in `ffi_commands.rs`/`ffi_queries.rs` in this crate — not invented fresh |

All six code changes are lint/documentation only. No function signature's
*behavior* changed, no schema changed, no wire format changed, no FFI ABI
changed (the `# Safety` additions are doc comments; the function signatures
and bodies are byte-for-byte identical to before).

## Verification actually performed this session

Toolchain: `rustc 1.97.1 (8bab26f4f 2026-07-14)` / `cargo 1.97.1`, installed
fresh via `rustup` to match `rust-toolchain.toml` exactly. `SQLX_OFFLINE=true`
throughout, using the repo's committed `.sqlx/` cache — no live database was
needed or used.

| Command | Result |
|---|---|
| `cargo clippy -p api-server --lib --bins -- -D warnings` | **Clean.** `result_large_err` confirmed gone; also caught and required a fix for a self-introduced `doc_lazy_continuation` violation in my own first draft of the `lib.rs` doc comment (see below) |
| `cargo clippy -p api-server --all-targets -- -D warnings` | **Clean** |
| `cargo clippy -p client-composition -p e2e --all-targets -- -D warnings` | **Clean.** Both are downstream dependents of `api-server`-as-a-library; checked because the `StorageBackendHandles` type alias and `ApiError` changes could plausibly have broken a consumer. Neither did. |
| `cargo clippy --workspace --exclude desktop-shell --exclude e2e -- -D warnings` | **Clean**, after the `worker` and `mobile-core` fixes above. `desktop-shell` excluded because this sandbox has no GTK/WebKit installed (consistent with Session 4 finding #16); `e2e` already verified separately above. |
| `cargo test -p api-server -p worker -p mobile-core --lib -- --test-threads=1` | **Pass.** 0 tests in each crate's `lib` target (consistent with prior audit note that `api-server` "has no tests of its own" — its behavior is exercised via the separate `e2e`/`team7-integration-tests` crates, which were not re-run in full this session; see below) |
| `cargo fmt --all -- --check` | **0 diffs**, workspace-wide |

### Self-correction, stated for the record

My first draft of the `lib.rs` rationale comment used clippy's markdown
bullet-list syntax with wrapped/indented continuation lines. `cargo clippy`
itself rejected this under `-D warnings` (`doc_lazy_continuation`) — a real
error, not a style nit clippy would silently ignore. I did not notice this
until running the verification command; it was not caught by inspection.
Fixed by writing each paragraph as a single unwrapped doc line. This is
recorded here rather than silently corrected, per standing project practice
of not treating verification as passed until actually rerun and green.

### What was NOT run this session, and why

- **`cargo clippy -p desktop-shell`** — this sandbox has no
  `libgtk-3-dev`/`libwebkit2gtk-4.1-dev`/`libsoup-3.0-dev` installed and none
  of this session's changes touch `desktop-shell` or anything it depends on,
  so it was judged out of scope to install a full GTK toolchain for.
- **Full `cargo test --workspace`** (integration/e2e suites, `team7-integration-tests`,
  chaos suite) — not re-run. This session's changes are lint-only in three
  small, mechanically-verified files; the full integration/e2e/chaos suites
  take real wall-clock time and disk (sandbox disk dropped from 10GB to
  4.7GB free over the course of this session's builds) and were judged
  unnecessary to re-verify lint-only changes with no behavioral delta. If a
  future session touches `api-server`, `worker`, or `mobile-core` again,
  those suites should be run before claiming full verification.
- **Android/iOS mobile builds** — `mobile-core`'s FFI *doc comments* were
  edited, not its Rust or Dart source; no mobile-side rebuild was needed to
  verify a clippy fix, and mobile toolchains are not installed in this
  sandbox regardless.

## Open items carried forward (not addressed this session, not silently dropped)

Everything else in `AUDIT_REGISTER.md` — H-01 through H-04 follow-ups
already marked DONE in earlier sessions, the `sqlx 0.7`/`rustls 0.21`
dependency-modernization item (H-04, still open), and anything not
explicitly touched above — is unchanged by this session and should be
read from `AUDIT_REGISTER.md` directly, not assumed resolved because this
document exists.
