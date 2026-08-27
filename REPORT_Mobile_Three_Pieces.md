# Report: Mobile Approval Authority, Class-Based Access Control, File Sharing

All work landed on `main` in the SMozaff/Onyx-Framework repository. Four commits, in order:

| Commit | Summary |
|---|---|
| `0400f81` | Piece 1 — mobile approval authority (`HierarchyCache` shared, `mobile_core_set_hierarchy`) |
| `1cbb0dc` | Piece 2 — class-based mobile access control, restrictive by default |
| `96d64fa` | Piece 3 — mobile file sharing (FFI upload/download + UI) |
| `83ee0b9` | Correction — `web-ui` was missed in Piece 2; fixed, plus a strengthened test |

---

## Piece 1 — Mobile Approval Authority

**The gap.** `mobile-core`'s `AppState` was built with `owner_authority: None`, so every Task/Mission approval command (`ApproveTask`, etc.) was unconditionally denied on mobile.

**Design decisions made, and why:**

- **Separate FFI call vs. extending `mobile_core_new`'s config** → a separate call, `mobile_core_set_hierarchy`. `mobile_core_new` must succeed (opening the local SQLite pool, applying migrations) before any hierarchy data can even exist, so it can't be part of construction-time config.
- **`HierarchyCache` splitting** → moved wholesale from `desktop-shell` into `client-composition::hierarchy_cache`, shared by both clients. `desktop-shell` is a binary crate and cannot be a library dependency of `mobile-core`, so the logic had to live somewhere both could reach.
- Added a `load_from_json` path alongside the existing HTTP `refresh`, since mobile's FFI layer has no `dio`/HTTP client of its own — the caller (Dart) supplies the JSON it already fetched. **Both `refresh` and `load_from_json` call one shared method, `replace_from_wire`** — one real implementation of id-parsing/map-building, not two.

**Code change** (`crates/mobile-core/src/lib.rs`):
```rust
owner_authority: Some(Arc::new(hierarchy_cache.clone()) as Arc<dyn api_server::OwnerAuthority>),
```
— changed from `None`. The cache starts empty (fail-closed: an empty cache authorizes no one) and is populated later via `mobile_core_set_hierarchy`, in place, without rebuilding `AppState`.

**Real, end-to-end test** (`crates/mobile-core/tests/hierarchy_authority_gate.rs`), through the actual `extern "C"` FFI boundary:
1. Owner creates a task and submits completion — ungated, succeeds.
2. Before any hierarchy is loaded, `ApproveTask` is denied for everyone (fail-closed default).
3. A hierarchy is loaded via `mobile_core_set_hierarchy`.
4. An unrelated stranger is still denied.
5. The task's real, cache-resolved direct manager succeeds.
6. A fresh `GetTask` query confirms the approval persisted (not just an in-memory result).

**Verification (re-run live during this report):**
```
cargo test -p client-composition --lib hierarchy_cache   → 8 passed, 0 failed
cargo test -p mobile-core --test hierarchy_authority_gate → 1 passed, 0 failed
```

**Unanticipated finding, disclosed rather than fixed:** FFI-mode mobile has no login/authentication mechanism at all — `organization_id`/`user_id` come from hardcoded placeholder UUIDs in `SharedPreferences`, never a server round-trip. The mechanism built here is correct and tested, but has no real call site in production until FFI-mode mobile gets an actual auth flow (out of scope, flagged as future work).

**Commit:** `0400f81` (this piece alone, not mixed with anything else).

---

## Piece 2 — Class-Based Mobile Access Control

**The one question asked before implementation, per the task's explicit requirement:** for an org with no `mobile_class_access` rows at all, should mobile login be permissive or restrictive by default? **Answered: restrictive.** An org with zero grants denies mobile login for every class until an admin explicitly grants one.

**Migration** (`migrations/postgres/20260108000000_add_mobile_class_access.up.sql`, SQLite counterpart identical in shape):
```sql
CREATE TABLE mobile_class_access (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL,
    user_class TEXT NOT NULL CHECK (user_class IN (
        'top_level_manager','senior_manager','team_leader','supervisor','staff'
    )),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (organization_id, user_class)
);
CREATE INDEX idx_mobile_class_access_organization_id ON mobile_class_access (organization_id);
```

**`LoginRequest` / login handler** (`crates/bins/api-server/src/routes/auth.rs`):
- `client_type: Option<String>` added — additive, only `Some("mobile")` triggers the gate.
- Gate logic (line ~132): `if payload.client_type.as_deref() == Some("mobile") && !user.is_admin { ... }` — checks `list_mobile_access` against the user's class before tokens are issued; denies with `403 MOBILE_ACCESS_RESTRICTED` if not granted. Admin bypasses unconditionally, matching the existing `require_class` Admin-bypass precedent elsewhere in the codebase.

**Admin endpoints** (`crates/bins/api-server/src/routes/admin.rs`):
- `GET`/`PUT /api/admin/mobile-access`, both gated by `require_admin(&state, &headers).await?` — identical guard to every other admin route in this file.
- `admin-shell` Settings page gained a `MobileAccessPanel` (checkbox per class).

**Every client, checked directly — one gap found and fixed:**

| Client | Sends `client_type`? |
|---|---|
| `desktop-shell` | ✅ `"desktop"` |
| `admin-shell` | ✅ `"admin"` |
| `mobile` (`net/auth.dart`) | ✅ `"mobile"` |
| `web-ui` | ❌ initially missed — **fixed in commit `83ee0b9`**, now sends `"web"` |

The original Piece 2 report claimed "every client" without having actually checked `web-ui` (a separate app at the repo root). That was inaccurate until the follow-up fix.

**Real verification — the exact scenario requested, run live:**

New test `excluded_class_denied_on_mobile_allowed_on_desktop_granted_class_allowed_on_both`:
- Grants only `"supervisor"` up front; `"staff"` is never granted.
- `staff` + `client_type: "mobile"` → **403 `MOBILE_ACCESS_RESTRICTED`**
- `staff` + `client_type: "desktop"` → **200**
- `supervisor` + `client_type: "mobile"` → **200**
- `supervisor` + `client_type: "desktop"` → **200**

```
cargo test -p api-server --test mobile_access_gate  → 2 passed, 0 failed
cargo test -p security-adapter                       → 25 passed, 0 failed
web-ui: npx tsc -b && npx vite build                 → both clean
```

**Commits:** `1cbb0dc` (original piece), `83ee0b9` (web-ui fix + strengthened test, after the gap was found on re-verification).

---

## Piece 3 — Mobile File Sharing

**Pre-implementation confirmation, per the plan's requirement:** checked `crates/bins/api-server/src/routes/` directly — there is no HTTP file upload/download route anywhere (only an unrelated CSV multipart import in `profiles/batch.rs`). Building that backend route would be new, unplanned server work, so it was left out of scope: the HTTP transport's `uploadFile`/`downloadFile` throw an explicit `UnsupportedError` rather than silently no-op'ing or faking success.

**What was built:**
- `mobile_core_upload_file` / `mobile_core_download_file` FFI functions (`crates/mobile-core/src/ffi_files.rs`), mirroring `desktop-shell`'s `upload_file`/`download_file` Tauri commands exactly — both sit on the same shared `FileUploadCoordinator`. Takes a filesystem path in (not raw bytes), for the same IPC-cost reason desktop-shell documents. MIME type hardcoded to `application/octet-stream`, matching desktop's own documented choice.
- Dart bridge bindings in `bridge.dart` (`OnyxMobile` implements both for real; `OnyxHttpApi` throws the explicit unsupported error).
- New `files.dart` screen added to the mobile bottom nav (path-based input; no file-picker package added since it couldn't be verified in this sandbox — flagged as a follow-up).

**Real, end-to-end tests** (`crates/mobile-core/tests/file_sharing.rs`), through the actual FFI boundary:
- Upload a real 10,000-byte file from disk, download it back by its returned content hash, assert byte-for-byte match.
- Upload a real 100 MiB + 1 byte file, assert it is rejected (confirms the existing `MAX_FILE_SIZE_BYTES` domain check, shared via `FileUploadCoordinator`, needed no duplicate here).

**Verification:**
```
cargo check -p mobile-core                                → clean
cargo clippy -p mobile-core --all-targets -- -D warnings  → clean
cargo test -p mobile-core --test file_sharing             → 2 passed, 0 failed
```

**Found and fixed while implementing, disclosed as unplanned:** `mobile/test/fakes.dart`'s `FakeOnyxApi` was already missing the `setHierarchy` override Piece 1 added to the `OnyxApi` interface — a real gap from that earlier piece, uncaught because this sandbox has no Dart toolchain to run `dart analyze`. Fixed alongside the new overrides.

**Commit:** `96d64fa`.

---

## What Was Not Verified (disclosed, not glossed over)

- No Dart/Flutter SDK exists in this sandbox: all Dart changes across all three pieces (`bridge.dart`, `onyx_http_api.dart`, `files.dart`, `fakes.dart`, `useAuth.ts`'s Dart-adjacent siblings) were hand-verified against existing file patterns but never compiled, analyzed, or run through `flutter test`.
- No real Android/iOS build or on-device test.
- No `desktop-shell`/`admin-shell` GTK-linked build in this sandbox (`gdk-3.0` pkg-config missing) — confirmed to be a pre-existing, unrelated sandbox limitation, not caused by any of this work (checked against unmodified `main` via `git stash`).
- Four pre-existing, unrelated `api-server` test-binary failures were found while running the full suite (`query_id_normalization`, `relay_switchboard`, `staff_loan_authorization`, `user_hierarchy_admin_routes`, `staff_profile_routes`, `team_leader_precheck_authorization`) — confirmed identical on unmodified `main`, not introduced by this work, not fixed (out of scope).
- No live click-through of the new `MobileAccessPanel` or `FilesScreen` UI in a running browser/app — only builds were verified.

All decisions, verification results, and disclosed gaps are also recorded in `DECISIONS.md` in the repository, entry by entry, in the project's established long-form style.
