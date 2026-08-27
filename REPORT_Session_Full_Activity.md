# Full Session Activity Report

Repository: `SMozaff/Onyx-Framework`, branch `main`. This report covers every
piece of work performed in this session, in chronological order, with real
commit hashes, what was verified, and what remains disclosed as unfinished.
Long-form detail for each piece also lives in `DECISIONS.md`, entry by entry,
in the project's established style — this file is the consolidated summary.

---

## 0. Raw-content verification (pre-existing fix, reviewed not authored this session)

Before any new work, the user asked for the exact, unedited source of five
items from an already-landed Task/Mission owner-authority-gate fix, to verify
it independently: `task_owner_authority_gate.rs`, the `OwnerCheck<A>` alias,
`AllowAllOwnerAuthority`, the `map_command_error` arm, and the full
`DECISIONS.md` entry. Pasted directly in chat, then published as an artifact
and sent as a downloadable file per follow-up requests.

No commit — this was a review of already-existing code (commit `4ae6091`,
already on `main` before this activity), not new work.

---

## 1. Two pending fixes verified and applied

**Commits:** `9c1b4bd` (mobile-core fix), `4ae6091` (auth gate — already
landed, confirmed present).

- **`crates/mobile-core/src/lib.rs`**: `mobile_core_new` was calling
  `AppState::new(pool, app_config)` without `.await` (a compile error —
  `AppState::new` is `pub async fn`), and its `AppStateConfig` literal was
  missing the required `blob_store_root` field. Fixed: added `.await` and
  derived `blob_store_root` from `db_path`'s parent directory, mirroring
  `desktop-shell`'s own `data_dir.join("blobs")` pattern.
- Verified: `cargo check --workspace` clean, `cargo build -p mobile-core`
  succeeded.

---

## 2. Release build matrix scoping (`.github/workflows/release.yml`)

**Commits:** `a3f4a15` (matrix narrowing), `079de71` (icon regeneration),
`7252ac1` (codesign fallback), `de31739` (notarization fix), `90f833f`
(DECISIONS.md entry).

**Task:** scope the release build matrix so server binaries
(`api-server`/`worker`/`migration-tool`/`sync-agent`) build **Windows-only**
(real deployment target: a Windows 10 office machine), while both desktop
apps (`desktop-shell`/"ONYX" and `admin-shell`/"ONYX Admin") continue building
for **Windows, macOS, and Debian Linux**.

**What was done:**
- Narrowed `release-binaries`' matrix from 3 OS entries to 1
  (`windows-2022`).
- Confirmed `release-images` (Docker/GHCR) was untouched and desktop jobs
  already covered all 3 OSes.
- Triggered **real** GitHub Actions runs (not just YAML review) via
  `workflow_dispatch`, and found and fixed **three separate real bugs** only
  visible from actual run logs:
  1. **Icon RGBA bug**: `.ico` files had RGB (not RGBA) embedded PNG frames,
     breaking Tauri's icon bundler on macOS. Fixed by regenerating via
     `tauri icon` for both `desktop-shell` and `admin-shell`.
  2. **Empty-string codesign identity**: GitHub Actions evaluates
     `${{ secrets.X }}` for a nonexistent secret as `""`, not unset —
     `Signing with identity ""` failed. Fixed with `|| '-'` fallback.
  3. **Empty-credential notarization attempt**: Tauri attempted notarization
     whenever `APPLE_ID` env var was present at all, even empty. Fixed with a
     new conditional `$GITHUB_ENV`-export step (a plain `env:` mapping can't
     conditionally omit a key).
- **Explicit decision surfaced via `AskUserQuestion`, not assumed**: macOS
  Apple Silicon vs. Intel scope — user confirmed Apple Silicon only.
  Disclosed plainly in `DECISIONS.md` that this was the project owner's real
  answer to a question raised, not an independent judgment call.
- Final verification: a fully **green** real CI run (`33003707428`) with all
  7 expected artifacts confirmed present by name and plausible size.

---

## 3. Mobile Piece 1 — real approval authority for FFI-mode mobile

**Commit:** `0400f81`.

**The gap:** `mobile-core`'s `AppState` was built with `owner_authority:
None`, so every Task/Mission approval command was unconditionally denied on
mobile.

**Design decisions made and justified:**
- Separate FFI call (`mobile_core_set_hierarchy`), not extended
  `mobile_core_new` config — hierarchy data isn't known until after login,
  which can't happen before construction.
- `HierarchyCache` moved from `desktop-shell` into
  `client-composition::hierarchy_cache`, shared by both clients (a binary
  crate can't be a library dependency of another crate). Added
  `load_from_json` alongside the existing HTTP `refresh`, with both routed
  through one shared `replace_from_wire` method.
- `owner_authority: Some(Arc::new(hierarchy_cache.clone()) as Arc<dyn
  api_server::OwnerAuthority>)` — changed from `None`.

**Verification:** new real end-to-end FFI test
(`hierarchy_authority_gate.rs`) — owner acts ungated, `ApproveTask` denied
before any hierarchy loaded (fail-closed default), a stranger still denied
after loading, the real cache-resolved manager succeeds, and a fresh query
confirms persistence. `cargo test -p client-composition --lib hierarchy_cache`
→ 8/8 passed; `cargo test -p mobile-core --test hierarchy_authority_gate` →
1/1 passed.

**Unanticipated, disclosed, not fixed:** FFI-mode mobile had no
login/authentication mechanism at all at this point — the mechanism built
here was correct but had no real call site until Piece 5 (below) added one.

---

## 4. Mobile Piece 2 — class-based mobile access control

**Commits:** `1cbb0dc` (implementation), `83ee0b9` (correction).

**Explicit question asked before implementing, per task requirement:** should
an org with no configured `mobile_class_access` rows default to permissive or
restrictive mobile access? **Answered via `AskUserQuestion`: restrictive.**

**What was built:**
- New `mobile_class_access` table (Postgres + SQLite migrations).
- `LoginRequest` gains optional `client_type`; only `client_type: "mobile"`
  triggers the gate. Denial returns `403 MOBILE_ACCESS_RESTRICTED`. Admin
  bypasses unconditionally, matching the existing `require_class` pattern.
- New admin-only `GET`/`PUT /api/admin/mobile-access` routes, gated by the
  same `require_admin` guard as every other admin route.
- `admin-shell` Settings page gains a `MobileAccessPanel`.
- Every first-party client updated to send its own `client_type`.

**Correction found on user-requested re-verification (`83ee0b9`):** the
initial report claimed "every client" sent `client_type`, but `web-ui` (a
separate app at the repo root, not checked initially) did not. Fixed, and a
stricter test scenario added
(`excluded_class_denied_on_mobile_allowed_on_desktop_granted_class_allowed_on_both`)
proving an excluded class is denied on mobile but allowed on desktop, while a
granted class succeeds on both.

**Verification:** `cargo test -p api-server --test mobile_access_gate` → 2/2
passed; `cargo test -p security-adapter` → 25/25 passed; `admin-shell`'s
`tsc -b`/`vite build` and `web-ui`'s `tsc -b`/`vite build` both clean.

---

## 5. Mobile Piece 3 — mobile file sharing

**Commit:** `96d64fa`.

**Pre-check performed, per task requirement:** confirmed `api-server` has no
HTTP file upload/download route at all (only an unrelated CSV batch-import
endpoint). Built out of scope; the HTTP transport's `uploadFile`/
`downloadFile` throw an explicit `UnsupportedError` rather than faking
success.

**What was built:** `mobile_core_upload_file`/`mobile_core_download_file` FFI
functions mirroring `desktop-shell`'s Tauri commands exactly (same shared
`FileUploadCoordinator`, path-in not bytes-in, same MIME-type default). Dart
bridge bindings and a new `files.dart` UI screen (path-based input; no
file-picker package added since it couldn't be verified in this sandbox).

**Verification:** two new real FFI end-to-end tests — upload-then-download
round-trips a 10,000-byte file byte-for-byte; a 100 MiB+1 file is rejected by
the existing `MAX_FILE_SIZE_BYTES` domain check. Both pass.

**Found and fixed in passing:** `mobile/test/fakes.dart`'s `FakeOnyxApi` was
missing the `setHierarchy` override Piece 1 had added — a real gap from an
earlier piece, uncaught because this sandbox has no Dart toolchain to run
`dart analyze`. Fixed here.

---

## 6. Mobile Piece 4 — real login for FFI-mode mobile

**Commit:** `4fa1dbd`.

**The gap:** FFI-mode mobile still had no login at all —
`organization_id`/`user_id` came from `SharedPreferences` with a hardcoded
placeholder-UUID fallback, zero server round-trip.

**Design decision, checked against the real current code first:** identity
resolution stays entirely in Dart — **zero Rust crate changes**.
`mobile_core_new` already takes `organization_id` as plain config with no
auth step, and `mobile-core`'s Rust side has no working `SecureStorage` at
all (`ffi_secure_storage.rs` — genuinely blocked, no JNI/Keychain bridge this
sandbox can build). A Rust-side login would need exactly that missing
mechanism.

**What was built:**
- New `ffi_login_screen.dart` — real `POST /api/auth/login`
  (`client_type: "mobile"`) via the same `OnyxHttpAuthApi` HTTP-mode already
  used, persisting real identity non-secretly and real tokens via a new
  `flutter_secure_storage`-backed `FfiSessionStorage`.
- New `OnyxHttpAuthApi.fetchHierarchyJson()` — the Dart-side hierarchy fetch
  Piece 1's own `DECISIONS.md` entry had explicitly deferred until a real
  caller existed.
- `main.dart::restartApp` gated on a real session existing; hardcoded
  placeholder fallback removed from both the app's startup path and the
  Android background sync task.
- "Sign out" action added to Settings.

**Two gaps found and disclosed, not fixed in this piece:**
1. `api-server` had no `/api/auth/refresh` route at all — persisted sessions'
   hierarchy refresh would silently stop working after ~1 hour.
2. `settings.dart`/`startup_error_screen.dart` still let someone manually
   type in arbitrary organization/user UUIDs, bypassing login.

**User's explicit severity ranking:** gap 2 (manual UUID entry) is a
**security hole** — fix first, immediately. Gap 1 (missing refresh) is a
**correctness bug**, lower severity — fix second, not urgent.

---

## 7. Fix #1 — removed the manual UUID-entry bypass (security hole)

**Commit:** `9536fae`.

- `OnyxController.saveSettings` no longer accepts `organization`/`user`
  overrides at all — replaced with `saveRelayEndpoint(String relay)`. Fixed
  at the method level, not just the widget tree, so no future caller could
  reach it either.
- `settings.dart` and `startup_error_screen.dart` both show identity
  **read-only** now; the only way to change it is a real login or the new
  "Sign out"/"Sign out and retry" action.
- Verified via `grep` that `saveSettings` had no other call site before
  renaming, and that no test referenced the removed fields.

---

## 8. Fix #2 — added `POST /api/auth/refresh` (correctness gap)

**Commits:** `a91bf34` (initial route), `1ff9013` (completion to a stricter
standard).

**Initial fix (`a91bf34`):** added `auth::refresh` — validates the refresh
token via the existing `validate_token` helper, re-confirms the user is
active, issues a new access+refresh pair, and **rotates** (revokes the old
refresh token). Two real end-to-end tests proved it worked, including
rotation and type-enforcement (an access token can't be used as a refresh
token).

**Completion (`1ff9013`), after a follow-up task asked for a stricter
standard:**
- **Confirmed exact access-token TTL from the real code**: `3600` seconds
  (exactly 1 hour) from `issue_token(&state, &user, "access", 3600)`'s two
  call sites — not assumed.
- **Proactive renewal added**: the earlier fix only refreshed reactively,
  once, at startup, after a failure. `OnyxController` now runs a 45-minute
  `Timer.periodic` for the life of a running FFI session (safely inside the
  1-hour TTL), only when `api is OnyxMobile`.
- **A real deterministic-expiry test added**:
  `access_token_that_has_actually_expired_is_rejected_and_refresh_recovers`
  — decodes a real, freshly-issued access token with the server's own
  signing key, rewrites only `exp`/`iat` into the genuine past, re-signs it
  (a validly-signed, genuinely-expired token, not a mock), confirms it's
  rejected by the exact `GET /api/users/hierarchy` endpoint mobile's
  hierarchy fetch uses, then confirms the refresh token recovers a working
  replacement against that same endpoint.
- **Checked, per explicit instruction, whether `desktop-shell` ever used a
  refresh path: confirmed it never has** — its `hierarchy_cache.refresh()`
  calls are an unrelated method (`HierarchyCache`, not tokens). Named
  explicitly as a real, adjacent, out-of-scope gap.
- **One more adjacent thing checked, found genuinely different, not
  conflated with the fixed bug:** `http_login_screen.dart` (HTTP-transport
  login) still has a free-text "Organization UUID" field. Confirmed via
  reading `routes/command.rs`/`routes/query.rs` directly that this is *not*
  exploitable — the server independently rejects `403 TENANT_MISMATCH` on
  every command/query, so a wrong org id there only ever produces failed
  requests, never impersonation (unlike the FFI case, which had no server
  round-trip at all).

**Verification:** `cargo test -p api-server --test auth_refresh` → 3/3
passed. Full `api-server` suite re-run: only the same pre-existing,
already-disclosed unrelated failure (see below). `clippy`/`check` clean
workspace-wide.

---

## 9. Branch cleanup

Checked `claude/onyx-pending-fixes-6l2hhk` (the branch originally designated
for this session's work) against `main`: it contained **zero commits not
already on `main`** — all work had ended up on `main` directly per later
explicit task instructions in this session. Deleted both the local and
remote copies of that branch; only `main` remains.

---

## Disclosed gaps still open (nothing here is silently treated as done)

1. **No Dart/Flutter SDK exists in this sandbox at all.** Every Dart change
   across every mobile piece this session — approval authority, class-based
   access, file sharing, real login, both follow-up fixes — was hand-verified
   against existing patterns and brace-balance-checked, but never compiled,
   analyzed, or run. Confirmed absent via `which dart`/`which flutter`.
2. **`flutter_secure_storage`** is a new native dependency (real Kotlin/Swift
   platform code) added for the login-token piece — cannot be built or linked
   in this sandbox at all.
3. **`ApprovalAggregate`** (staff-loan/list-verification kind, unrelated to
   Task/Mission approval) is not registered in `client-composition::app_state`
   anywhere — found while investigating the Approvals screen, never revisited.
4. **No real Android/iOS build or on-device test** of anything mobile-related,
   ever, this session.
5. **HTTP-mode file sharing** throws `UnsupportedError` — `api-server` has no
   HTTP file upload/download route; out of scope.
6. **No file-picker package** in the Files screen — path typed by hand.
7. **`desktop-shell` has never used a token refresh path** — same ~1-hour
   access-token ceiling mobile had before Fix #2, still present for desktop.
8. **`http_login_screen.dart`'s free-text "Organization UUID" field** —
   confirmed not exploitable (server enforces tenant match independently),
   but still worth a second look someday.
9. **Four pre-existing `api-server` test-binary failures**
   (`query_id_normalization`, `relay_switchboard`, `staff_loan_authorization`,
   `user_hierarchy_admin_routes`, `staff_profile_routes`,
   `team_leader_precheck_authorization`) — confirmed identical on unmodified
   `main` via `git stash`, not caused by anything this session did, never
   fixed (out of scope each time they were noticed).
10. **`desktop-shell`/`admin-shell` cannot be built in this sandbox at all**
    (missing `gdk-3.0`/GTK pkg-config) — a sandbox limitation, not a code
    defect, confirmed against unmodified `main`.

---

## Commit index (this session, chronological)

| Commit | Summary |
|---|---|
| `9c1b4bd` | Fix mobile-core `AppState::new` await + missing `blob_store_root` |
| `4ae6091` | (pre-existing) Task/Mission approval direct-manager authority gate |
| `a3f4a15` | Narrow server-binaries release matrix to Windows only |
| `079de71` | Regenerate app icons (RGBA fix for macOS bundling) |
| `7252ac1` | Fall back to ad-hoc macOS code signing when no cert configured |
| `de31739` | Don't attempt macOS notarization with empty Apple credentials |
| `90f833f` | DECISIONS.md: release build matrix scoping and verification |
| `0400f81` | Mobile Piece 1: real owner authority via shared `HierarchyCache` |
| `1cbb0dc` | Mobile Piece 2: class-based mobile access control |
| `96d64fa` | Mobile Piece 3: mobile file sharing (FFI + UI) |
| `83ee0b9` | Fix `web-ui` missing `client_type`; strengthen access-control tests |
| `d2cb9f7` | Add session report (Pieces 1–3) |
| `4fa1dbd` | Mobile Piece 4: real login for FFI-mode mobile |
| `9536fae` | Fix #1: remove manual UUID-entry bypass (security hole) |
| `a91bf34` | Fix #2: add `POST /api/auth/refresh` (initial) |
| `1ff9013` | Fix #2: proactive renewal + real-expiry test (completion) |

All of the above are on `main` at `origin/github.com/SMozaff/Onyx-Framework`.
