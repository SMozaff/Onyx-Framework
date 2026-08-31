# ONYX — Decisions Log (Phase 1: Desktop & Web Completion)

This file records decisions and provenance for work done under
`PLAN_Desktop_Web_Completion.md`. Historical decisions from Increments
1–8 (Team Prompts 1–8) are tracked separately in the project's original
`DECISIONS.md`/`DECISIONS_1-5.md`/`DECISIONS_Team_*.md` documents and are
not duplicated here — this file starts fresh at the point Phase 1 work
began.

---

## `policy-domain` — Complete

New crate: `crates/domains/policy-domain`. Source: Blueprint Part I
§4.18. Built per the explicit product decision to implement the **full**
Policy bounded context (not a toggles-only subset) — see
`PLAN_Desktop_Web_Completion.md` §7 item 2.

**Aggregates:** `Policy` (versioned rule set + scope), `LegalHold`
(retention override lifecycle). Independent aggregate roots — a hold can
be applied without a policy version driving it (§4.18.8: Compliance
Officer may apply ad hoc), so coupling their lifecycles would force an
artificial policy to exist for every hold.

**Commands implemented:** `CreatePolicy`, `CreatePolicyVersion`,
`PublishPolicyVersion`, `EvaluatePolicy`, `RegisterViolation`,
`RetirePolicy`, `ApplyLegalHold`, `ReleaseLegalHold`.

**Invariants enforced and tested** (§4.18.4):
- Policy evaluation is deterministic for identical inputs and policy
  version (`evaluation_is_deterministic_for_identical_inputs`).
- Published policy versions are immutable — a new draft never mutates
  the currently-effective published version
  (`published_version_is_immutable_a_new_draft_is_a_new_version`).
- Concurrent policy revisions require controlled publication — only one
  draft may be pending at a time (`second_draft_while_one_pending_is_rejected`).
- Policy changes never rewrite historical decisions — old
  `PolicyVersionRecord`s are retained in `Policy.versions`, never removed
  or mutated once published.

**Explicit design choices:**
- `PolicyRule` is `{rule_type, key: String, value: serde_json::Value}` —
  deliberately opaque rather than a closed Rust enum per possible
  setting, matching `communication_domain::RedactionReason`'s
  free-text precedent. New admin-configurable settings do not require a
  crate release.
- `HoldTarget.target_type` is an owned `String`, not `&'static str` —
  required for `Deserialize` to work on rehydrated/persisted events
  (caught by `cargo build`, not assumed).
- **Not included:** Quota & Rate Governance (§4.18.10 —
  `RateLimitPolicy`/`QuotaLedger`). Flagged, not silently omitted — no
  Phase 1 UI or workflow drives it; would be unused surface area if
  added now.
- Authority: follows the exact same stub precedent as
  `mission-domain`/`work-domain`/`communication-domain`/`file-domain` —
  `decide()` checks only `context.authority.is_authorized(...)`. Real
  per-role checks (e.g. "only Manager/Admin may publish a version") are
  deferred to the composition root, same as every other domain.

**Verification:**
```
cargo build --package policy-domain            # clean
cargo test --package policy-domain             # 22 tests, 0 failures
cargo clippy --package policy-domain --all-targets -- -D warnings   # clean
```

**Not yet done (tracked separately in `PLAN_Desktop_Web_Completion.md`):**
wiring into `client-composition`'s command/query registries, a
persistence adapter (SQLite/Postgres repository), and the admin
Settings UI (Desktop + Web).

---

## Manager role (`is_manager`) — Complete

Per the explicit decision in `PLAN_Desktop_Web_Completion.md` §7 item 3:
a distinct Manager role, separate from Admin, narrower permissions — not
a reused `is_admin` flag, not a full multi-tier role/permission system.

**Additive, per `UserStore`'s own documented extension path**
(`security_application::ports::user_store` explicitly anticipated this:
"Adding [roles] later is an additive change to `UserRecord`"). Every
existing `is_admin` check is untouched.

**Changed:**
- New migrations: `20260106000000_add_manager_role.{up,down}.sql`
  (Postgres + SQLite) — `is_manager BOOLEAN/INTEGER NOT NULL DEFAULT
  FALSE/0`, additive column.
- `security_application::ports::user_store`: `UserRecord.is_manager`,
  `NewUser.is_manager`, new `UserStore::set_manager()` trait method.
- `security_adapter::user_store`: both `PostgresUserStore` and
  `SqliteUserStore` updated (row mapping, INSERT, SELECT, new
  `set_manager` impl) — including the in-memory SQLite test harness's
  ad-hoc `CREATE TABLE`, which is a second, easy-to-miss schema copy
  (caught by `cargo test`, not assumed).
- `api_server::routes::admin`: `UserDto.is_manager`,
  `CreateUserRequest.is_manager`, new `SetManagerRequest` DTO, new
  `require_manager_or_admin()` guard (distinct from `require_admin` —
  narrower, for Policy/Settings routes specifically), new
  `POST /api/admin/users/:id/manager` route (admin-only, so a Manager
  cannot grant itself or others additional scope).

**Explicit design choice:** `is_admin` and `is_manager` are independent
booleans, not a ranked hierarchy — an Admin is not "a bigger Manager."
`require_manager_or_admin` accepts either because every deployment needs
at least one account that can do everything, not because Manager is
formally a subset of Admin.

**Flagged, not yet used:** `require_manager_or_admin` has no caller yet
— added now (`#[allow(dead_code)]`, documented why) so the Policy/
Settings routes, when built, have it ready rather than reusing the
too-broad `require_admin` or omitting a guard.

**Verification:**
```
cargo build --package security-application --package security-adapter --package api-server   # clean
cargo test --package security-adapter    # 15 tests, 0 failures (incl. new set_manager test)
cargo clippy --package api-server --package security-application --package security-adapter \
  --all-targets -- -D warnings           # clean
```

---

## Communication domain extensions (Supergroup/SubTeam, ConnectionRequest) — Complete

Per `PLAN_Desktop_Web_Completion.md` §7.1's confirmed decisions:
Supergroup is a Communication-domain concept (not an Organization
structural tier), an Organization may contain multiple Supergroups,
sub-team membership requires supergroup membership first, and
ConnectionRequest is a new User-to-User aggregate independent of any
Conversation.

**`ConversationType`** gained two variants: `Supergroup` and `SubTeam`.
`Conversation` gained a `parent_supergroup: Option<ConversationId>`
field — `Some` iff `conversation_type == SubTeam`, validated at
`Conversation::create()` (rejects a `SubTeam` with no parent, and
rejects a parent on any other type).

**Enforced invariant:** `AddMember` on a `SubTeam` conversation checks a
new `is_parent_supergroup_member: bool` field on the `AddMember` command
and rejects with `ConversationError::NotSupergroupMember` if `false`.
This crate cannot check the parent's actual membership itself — per §5.2
("strong consistency exists only within one aggregate"), a pure
`decide()` has no cross-aggregate read capability — so the composition
root (which loads both the sub-team and its parent supergroup) is
trusted to supply the answer, the same pattern `context.authority`
already uses for a pre-verified input.

**New aggregate: `ConnectionRequest`** (`Pending` → `Accepted` /
`Declined` / `Revoked`, all terminal). Sender is the requesting actor;
only the recipient may accept/decline, only the sender may revoke —
enforced in `decide()`, tested for both the authorized and unauthorized
actor on each transition.

**Serde compatibility, caught and fixed (not assumed):** the new
`AddMember.is_parent_supergroup_member` field is a plain `bool`, which
serde treats as **required** on deserialize unless annotated —
confirmed with a standalone reproduction before assuming otherwise, then
fixed with `#[serde(default)]` (defaults to `false`, fail-closed) and
locked in with two regression tests
(`add_member_json_without_new_field_defaults_to_false`,
`create_conversation_json_without_parent_supergroup_defaults_to_none`)
so a JSON payload predating this change still deserializes.

**Explicit design choices:**
- `ConnectionRequestId`, `ConnectionRequestStatus`, `ConnectionRequestCommand`,
  `ConnectionRequestEvent`, `ConnectionRequestError` all follow the same
  create()/decide()/apply() + from_created_event() pattern every other
  aggregate in this workspace uses — no new pattern introduced.
- Recipient resolution (username/ID/email → `UserId`) is explicitly
  **not** this crate's concern — `SendConnectionRequest` takes an
  already-resolved `UserId`, matching how every other aggregate receives
  resolved kernel ids rather than performing its own identity lookups.
  That resolution is `client-composition`'s/`api-server`'s job when this
  gets wired in.

**Verification:**
```
cargo build --package communication-domain --package client-composition   # clean
cargo test --package communication-domain    # 45 tests, 0 failures (28 pre-existing + 17 new)
cargo clippy --package communication-domain --package client-composition \
  --all-targets -- -D warnings               # clean
```

**Not yet done (tracked separately in `PLAN_Desktop_Web_Completion.md`):**
`ConnectionRequest` is not yet wired into `client-composition`'s
command/query registries, `api-server`'s command/query routes, or any
persistence adapter. The existing `Conversation`/`Message` wiring is
unaffected — every pre-existing call site supplies `None`/`false` for
the new fields, which is a documented no-op for every invariant this
change added.

**Known environment limitation (not a code defect):** `cargo build
--package desktop-shell` fails in this sandbox on a missing system
library (`gdk-3.0`/GTK/WebKit2GTK, required by Tauri's Linux target) —
package-mirror fetch failures prevented installing them here. This is
unrelated to the Rust-level changes above, which were verified at the
`client-composition` layer (desktop-shell's actual logic dependency)
and built clean. Desktop-shell itself should be verified in the
project's own devcontainer/Codespace (which provisions these system
libraries, per `README.md`), not assumed.

---

## `client-composition` wiring — Complete

Wires `policy-domain` (`Policy`, `LegalHold`) and `communication-domain`'s
new `ConnectionRequest` aggregate into the real composition root
(`AppState`), following the exact existing pattern every other aggregate
(Mission, Task, Conversation, Message, FileAsset, UploadSession) already
uses — no new pattern introduced:

- `policy-domain` added as a workspace dependency of `client-composition`
  (was missing; caught by `cargo build`, not assumed).
- New `PolicyCreationHandler`/`PolicyDecisionHandler`,
  `LegalHoldCreationHandler`/`LegalHoldDecisionHandler`,
  `ConnectionRequestCreationHandler`/`ConnectionRequestDecisionHandler` —
  each a thin wrapper around `Aggregate::create()` /
  `api_server::handle_command::<Aggregate, _, _, _>`, identical in shape
  to the six existing handler pairs.
- `AppState::new`: three new `SqliteRepository` instances
  (`"policy"`, `"legal_hold"`, `"connection_request"` — generic
  aggregate-type-scoped storage, same as every existing repo), full
  command registration (`CreatePolicy`/`ApplyLegalHold`/
  `SendConnectionRequest` as creation commands; every other variant as
  decision commands, mirrored from each domain's own command enum), and
  three new query registrations (`GetPolicy`, `GetLegalHold`,
  `GetConnectionRequest`).

**Verified end-to-end against real SQLite** (not just unit-tested in
isolation): two new integration tests in `tests/app_state_wiring.rs`,
matching the existing Conversation/Message test's shape —
`app_state_new_wires_policy_commands_end_to_end` (CreatePolicy →
CreatePolicyVersion → PublishPolicyVersion → GetPolicy, asserting the
published state persists and round-trips) and
`app_state_new_wires_connection_request_commands_end_to_end`
(SendConnectionRequest → GetConnectionRequest, asserting `Pending`
status persists).

**Verification:**
```
cargo build --package client-composition   # clean
cargo test --package client-composition    # 30 tests, 0 failures (28 pre-existing + 2 new end-to-end)
cargo clippy --package client-composition --all-targets -- -D warnings   # clean
cargo build --package api-server           # clean (sibling crate, unaffected but reverified)
```

**Not yet done (tracked separately in `PLAN_Desktop_Web_Completion.md`):**
`api-server`'s own command/query routes (the ones Web talks to over
HTTP) are a **separate** registry from `client-composition`'s — see the
"Web Mission/Task write path" finding from earlier in this plan.
`api-server` currently only supports `notification`/`approval` commands
and `mission`/`task`/`timeline`/`notification`/`approval`/`report`/
`dashboard` queries; none of Policy/Communication's new capabilities are
exposed there yet, and per the confirmed decision, Web does not get
Communication features until Phase 2 regardless. Desktop UI (Messaging,
File-sharing, Settings screens) is the remaining Phase 1 work — the full
backend command/query surface for all of it is now wired and verified.

---

## `BlobStore` port + `local-blob-storage` + `FileUploadCoordinator` — Complete

**The gap:** confirmed by search before building anything — `file-domain`
tracks upload metadata (chunk hashes, sizes, sequencing) but never
touches actual file bytes, by design (no I/O in a domain crate). Nothing
in the delivered workspace stored real file content anywhere. Flagged
to the user rather than silently built around; user chose "build a real
local-disk adapter now" over a metadata-only shell.

**New port:** `query_application::BlobStore` (`put`/`get`/`exists`/
`delete`, keyed by content hash via a new `BlobKey` newtype). Content-
addressed by design — two uploads with identical bytes share one stored
blob. Deliberately does not depend on `file-domain` (domain crates don't
depend on each other, §5.2); speaks in a hash-string vocabulary the
composing handler is responsible for keeping consistent with
`file_domain::value::ContentHash`.

**New crate: `local-blob-storage`** — a real, working local-disk
implementation. Content-addressed, sharded by 2-hex-char prefix
(avoids one huge flat directory). Durable writes via temp-file-then-
`rename()` (atomic on POSIX, same filesystem/shard dir). Idempotent
`put` (matches the port's documented contract — a retried write after a
crash must not fail). Path-traversal-safe (`BlobKey` is caller-supplied,
ultimately from a claimed hash before this store has verified anything,
so it is validated as hex-only before ever touching the filesystem, not
trusted as a safe path component). **9/9 tests pass, clippy clean.**

**New: `FileUploadCoordinator`** (`client-composition::file_upload`) —
the piece that actually ties bytes to the domain commands. Given raw
bytes + a file name + MIME type: creates a `FileAsset`, starts an
`UploadSession`, chunks the content (4MiB chunks), computes each
chunk's real SHA-256 hash, writes each chunk to `BlobStore`, drives
`AppendChunk` per chunk and `FinalizeUpload` through
`api_server::handle_command` (reusing its version/epoch-conflict and
event-envelope logic rather than hand-rolling it), stores the whole
file under its own content hash too (current readers download by
overall hash, not by chunk), and records the result via `CreateVersion`
on the `FileAsset`. Also exposes `download(content_hash)`.

**Explicit design choices:**
- Lives in `client-composition`, not `desktop-shell` — per the explicit
  product decision that this should be a shared, composable piece
  (consistent with how every other cross-cutting concern in this crate
  works), not duplicated per native client.
- A dedicated coordinator rather than more `CommandRegistry` entries —
  a real upload is an inherently multi-step, byte-carrying sequence
  that doesn't fit the one-command-per-JSON-envelope shape every other
  registered handler uses; documented in the module's own doc comment
  rather than silently forcing a bad fit.
- Uses a fresh, empty `IdempotencyStore` per coordinator (not
  `AppState`'s shared one) — each step within one upload already uses a
  distinct `OperationId` by construction, so cross-step idempotency
  caching would never hit; documented as a deliberate no-op, not an
  oversight.
- Does not yet support uploading a new version of an *existing*
  `FileAsset` — flagged in the coordinator's own doc comment, not
  silently omitted; no Phase 1 UI workflow needs it yet.

**Breaking change, fully propagated:** `AppState::new()` is now `async`
(`LocalBlobStore::open` needs to create its root directory, which is
I/O). Updated at every call site: `desktop-shell/src/lib.rs`'s one real
caller, and all 8 call sites across `client-composition`'s own test
suite. Every existing caller already ran inside an async context (Tauri
commands, `#[tokio::test]`), so this was a mechanical `.await` addition,
not a structural change to how `AppState` is used.

**Verified end-to-end** with two new real-content integration tests in
`tests/app_state_wiring.rs`:
`file_upload_coordinator_round_trips_real_content` (single-chunk upload,
asserting the downloaded bytes exactly match what was uploaded) and
`file_upload_coordinator_handles_multi_chunk_content` (content larger
than one 4MiB chunk, asserting correct reassembly across chunk
boundaries).

**Verification:**
```
cargo build --package local-blob-storage    # clean
cargo test --package local-blob-storage     # 9 tests, 0 failures
cargo build --package client-composition --package api-server   # clean
cargo test --package client-composition     # 30 tests, 0 failures (28 prior + 2 new file-upload end-to-end)
cargo clippy --package client-composition --package local-blob-storage \
  --all-targets -- -D warnings              # clean
```

---

## Desktop UI + real file storage — Complete

### `BlobStore` port + `local-blob-storage` adapter (new)

**Gap found, not assumed:** `file-domain` tracks upload *metadata* only
(chunk sequencing, sizes, hashes) and by design never sees file bytes.
A search confirmed **no blob/byte storage existed anywhere in the
workspace** — no S3 adapter, no local store. A File-sharing UI would
have been a non-functional shell without this. Product decision was to
build real storage now.

- **Port:** `query_application::ports::blob_store` — `BlobStore` trait
  (`put`/`get`/`exists`/`delete`), content-addressed via a `BlobKey`
  newtype. Deliberately does **not** depend on `file-domain` (domain
  crates don't depend on each other, §5.2), so it speaks a
  domain-agnostic vocabulary.
- **Adapter:** new `crates/infrastructure/local-blob-storage`.
  Content-addressed, sharded by the key's first 2 hex chars (avoids an
  unbounded flat directory), **atomic writes** via temp-file + `rename`
  in the same shard dir (POSIX atomicity requires same filesystem — the
  reason the temp file isn't in a shared `/tmp`). Idempotent `put`,
  non-error `delete` of a missing key, and path-traversal rejection
  (keys must be hex — a caller-supplied key is never trusted as a path
  component).
- **Not built (flagged):** an S3-compatible adapter for a future
  Web/cloud backend. Web file sharing is permanently out of scope per the
  confirmed product decision, and no other consumer needs it today.

### `FileUploadCoordinator` (new, in `client-composition`)

Ties the two halves together: `CreateFileAsset` → `StartUpload` → N ×
(`SHA-256` chunk hash → `BlobStore::put` → `AppendChunk`) →
`FinalizeUpload` → `CreateVersion`. Reuses `api_server::handle_command`
for every decision-path step rather than hand-rolling version/epoch
conflict handling and event enveloping.

- Not a `CommandRegistry` entry: an upload is a multi-step sequence
  needing raw bytes and per-step version threading, which the generic
  JSON `CommandEnvelope` path can't carry well.
- 4 MiB chunks. Stores both per-chunk blobs and one whole-file blob
  keyed by the overall content hash (readers download by the latter);
  the duplication is documented and acceptable under the 100 MB cap.
- **Verified end-to-end with real content**, including a 9 MiB
  multi-chunk case that exercises the chunk loop and version threading:
  `file_upload_coordinator_round_trips_real_content`,
  `file_upload_coordinator_handles_multi_chunk_content`.

### `AppState::new` is now `async` (breaking change)

Required because `LocalBlobStore::open` performs directory-creation I/O.
All 7 call sites updated (6 tests + desktop-shell). Every one already ran
inside an async context, so this was a mechanical `.await` addition, not
a structural change. New `AppStateConfig.blob_store_root` field;
desktop-shell points it at `{app_data_dir}/blobs`, alongside
`onyx.sqlite`.

### Desktop UI pages (new)

Three new pages, wired into `App.tsx` routes and `MainLayout`'s nav:

- **`Messaging.tsx`** — conversations (all four `ConversationType`s
  including the new `Supergroup`/`SubTeam`), member management with the
  "supergroup membership required first" invariant surfaced in the UI,
  message post/edit/redact/react, and a Connections panel for
  `ConnectionRequest` send/accept/decline/revoke.
- **`Files.tsx`** — upload by path (via the new `upload_file` Tauri
  command), version list, download to disk, access grant/revoke,
  quarantine, archive.
- **`Settings.tsx`** — full Policy administration per the decision to
  expose all of §4.18: create policy, author multi-rule draft versions,
  publish, evaluate, retire, plus an independent Legal Hold panel
  (apply/release). Rule authoring is open key/value rather than a
  hard-coded settings list, matching `PolicyRule`'s deliberately open
  shape.

**Visual tone:** professional/utilitarian, reusing the existing
`onyx-*` dark design tokens — explicitly *not* a consumer-chat
aesthetic, per the corrected product direction.

**Role gating (flagged, not faked):** `Settings.tsx` does **not** gate
itself by the Manager role. The role exists end-to-end on the backend,
but this shell has no real authenticated session yet (`useSession` is a
documented placeholder), so a client-side check against a fabricated
identity would be security theatre. The backend check is authoritative
and unaffected.

### New Tauri commands

`upload_file(path, organizationId, userId, deviceId)` and
`download_file(contentHash, destinationPath)`. Both take/return
filesystem **paths** rather than bytes — shipping up to 100 MB through
Tauri's JSON-encoding IPC would cost an extra copy plus encoding
inflation for no benefit, since the native side can read/write directly.
MIME type is currently a hardcoded `application/octet-stream`: no
sniffing library is a workspace dependency, and extension-guessing would
silently mislabel files. Flagged as a follow-up rather than faked.

### App icons

Replaced the placeholder `icons/icon.{ico,png}` with the supplied ONYX
dark-variant logo. **`icon.png` had to be converted to RGBA** — Tauri's
`generate_context!` macro rejects a non-RGBA PNG at compile time, which
the source asset (RGB, no alpha) triggered. Caught by an actual build,
not assumed.

### Verification

```
cargo check   --package desktop-shell                    # clean
cargo test    --package client-composition               # 30 tests, 0 failures
cargo test    --package local-blob-storage               #  9 tests, 0 failures
cargo test    --package policy-domain                    # 22 tests, 0 failures
cargo test    --package communication-domain             # 45 tests, 0 failures
cargo clippy  --package desktop-shell --package client-composition \
              --package local-blob-storage --package query-application \
              --all-targets -- -D warnings               # clean
npx oxlint                     (desktop-shell/ui)        # 0 warnings, 0 errors
npx tsc                        (desktop-shell/ui)        # 0 errors *
```

\* The project's `tsc -b` reports pre-existing `react-router-dom` module
resolution errors under TypeScript 6.0.3 + `moduleResolution: "bundler"`,
affecting **every** page file including ones untouched by this work
(`App.tsx`, `Missions.tsx`, `Tasks.tsx`, `Dashboard.tsx`, `main.tsx`).
Confirmed pre-existing and environmental, not introduced here; the new
pages typecheck with **zero** errors under an otherwise-identical
isolated config (`strict`, `noUnusedLocals`, `noUnusedParameters` all
on). Worth resolving separately — likely a TS/router version pairing
issue in the toolchain, not a code defect.

**Environment note:** the GTK/WebKit2GTK system libraries Tauri's Linux
target needs are now installed in this sandbox (`libgtk-3-dev`,
`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`,
`libjavascriptcoregtk-4.1-dev`), so `desktop-shell` is verifiable here —
the blocker noted in earlier sections of this document is resolved.

---

## Phase A — User Hierarchy: class + reporting-line data model — Complete

Per `DESIGN_User_Hierarchy_Chain_of_Authority.md` §2-§2.3 and
`IMPLEMENTATION_PLAN_User_Hierarchy.md` §2. Supersedes `is_manager`
(kept during a deliberate two-step migration window — see below) with a
real class hierarchy and a reporting-line tree.

**New migrations** (`20260107000000_add_user_class_hierarchy`, Postgres
+ SQLite): additive `class` (nullable TEXT, Postgres `CHECK`-constrained
to the five confirmed class values) and self-referential
`parent_user_id` on `users`. Deliberately does **not** drop
`is_manager` in this migration — per the plan's own flag, the backfill
decision (what class do existing `is_manager = true` users become) is a
real operational judgment call, not something to silently pick a
default for. Flagged, not resolved, in this pass.

**Found and documented a real, pre-existing gap while writing the
SQLite migration:** SQLite foreign keys are never enabled anywhere in
this codebase (no connection sets `PRAGMA foreign_keys = ON`), so
`parent_user_id`'s `REFERENCES` clause is decorative on SQLite only —
confirmed by grep before writing the comment, not assumed. This makes
the application-level parent-existence check in `SqliteUserStore`
load-bearing, not a backstop, unlike on Postgres where the database
itself also enforces it.

**Port (`security-application`):** new `UserClass` enum (exactly the 5
confirmed classes — Top-level Manager, Senior Manager, Team Leader,
Supervisor, Staff; Admin and Allfather are deliberately **not**
variants — see the enum's doc comment for why), with stable
`as_str()`/`parse()` wire format. `UserRecord`/`NewUser` gained
`class`/`parent_user_id`. New `UserStoreError` variants
`ParentNotFound`/`ParentCycle`. New trait methods `set_class`/
`set_parent`. `is_manager` marked `#[deprecated]` in doc comments
(kept operational, not removed).

**Adapters (`security-adapter`):** both Postgres and SQLite fully
implement the new port surface, including real cycle-detection logic
(`is_ancestor`, a bounded ancestor-chain walk, fail-closed on excessive
depth) — no database constraint can express "no cycles in a
self-referential tree," so this is genuinely application-level, tested
application-level, not assumed correct. **24/24 tests pass**, including
`set_parent_rejects_indirect_cycle` (A→B→C, then reject A's parent
becoming C — exercises the ancestor walk, not just the trivial
self-reference case). Clippy clean.

**`api-server`:** new `SetClassRequest`/`SetParentRequest` DTOs (both
take `Option<String>` **without** `#[serde(default)]` on the inner
option's clearing semantics — an explicit `null` is required to clear a
class/parent, distinct from omitting the field, since clearing is a
real, deliberate action). New admin-only `POST /api/admin/users/:id/class`
and `POST /api/admin/users/:id/parent` routes. New `require_class`
permission guard — deliberately an explicit allow-list per call site
rather than a `>=` rank comparison, because `UserClass`'s own ordering
is not a true linear hierarchy for permission purposes (documented on
the enum itself). Not yet called anywhere (`#[allow(dead_code)]`,
matching the precedent already set by `require_manager_or_admin`) —
ready for Phases C-E's not-yet-built routes.

**New end-to-end HTTP test suite**
(`tests/user_hierarchy_admin_routes.rs`), matching the project's own
"compiling is not evidence" testing philosophy
(`relay_switchboard.rs`'s stated approach) — 7 tests against a real
bound server and real SQLite: class set at creation, class update and
explicit-null clearing, invalid-class rejection, parent linking,
self-reference rejection, nonexistent-parent rejection, and
unauthenticated-request rejection. **7/7 pass.**

**Verification:**
```
cargo build --package security-application                                  # clean
cargo build --package security-adapter                                      # clean
cargo test --package security-adapter                                       # 24 tests, 0 failures
cargo clippy --package security-adapter --package security-application \
  --all-targets -- -D warnings                                              # clean
cargo build --package api-server                                            # clean
cargo test --package api-server                                             # 10 tests, 0 failures (3 pre-existing + 7 new)
cargo clippy --package api-server --all-targets -- -D warnings              # clean
```

**Not yet done (tracked in `IMPLEMENTATION_PLAN_User_Hierarchy.md`):**
the `is_manager` → `class` backfill decision and follow-up migration
that actually drops `is_manager`; Phase B (org-provisioning action —
resolved 2026-08-15, see this file's later entry, no longer blocked);
Phase C (staff loans); Phase D (`todo-domain` crate); Phase E
(escalation — scope also resolved 2026-08-15); Phase F (UI for all of
the above).

---

## Phase A follow-up fixes — Complete

Two gaps flagged (not resolved) when Phase A first shipped. Both
resolved 2026-08-13 per explicit person decisions.

### SQLite foreign-key enforcement — fixed

**Decision:** turn it on (low effort, real defense-in-depth benefit, no
identified downside since nothing was relying on FK violations being
silently tolerated).

Both real production SQLite pool-creation sites now set
`SqliteConnectOptions::foreign_keys(true)` — this is a per-connection
setting, not database-wide, so it must be attached to the
`SqliteConnectOptions` every pooled connection is opened with, not run
as a one-time `PRAGMA` query against a single connection (which would
be lost the moment the pool opened a second connection):
- `api_server::routes::mod` — the shared `api-server`/Web pool.
- `desktop_shell::lib` — desktop's own local `onyx.sqlite` pool. Also
  switched from `SqlitePoolOptions::connect(url_string)` to
  `.connect_with(SqliteConnectOptions)`, since the URL-string form has
  no way to attach this option.

**Caught and fixed a real test-fixture bug while verifying this, not
just assumed the fix worked:** `security-adapter`'s in-memory test
fixture initially used `SqliteConnectOptions::new().filename(":memory:")`,
which — unlike the special-cased `"sqlite::memory:"` URL string sqlx
recognizes — gives **every pooled connection its own separate, empty
in-memory database**. With the pool's default (>1) connection limit,
this meant `CREATE TABLE` landed on one connection while a later query
could hit a different, empty one — surfaced immediately as `"no such
table: main.users"` across 16 of 24 tests the first time this was run,
not discovered in review. Fixed with a named, shared-cache in-memory
database (`file:...?mode=memory&cache=shared`) plus `max_connections(1)`
as a second layer of protection against the same class of bug.

**Verification:**
```
cargo test --package security-adapter                          # 24 tests, 0 failures (FK enforcement genuinely active)
cargo clippy --package security-adapter --all-targets -- -D warnings   # clean
cargo build --package api-server                                # clean
cargo test --package api-server                                 # 10 tests, 0 failures — including set_parent_rejects_nonexistent_parent_over_http, which now exercises real DB-level rejection, not just the application-level check
cargo clippy --package api-server --all-targets -- -D warnings  # clean
cargo check --package desktop-shell                             # clean
```

### `is_manager` → `class` backfill — decided, documented, not automated

**Decision:** manual reclassification by an Admin, not auto-promotion.
Rationale given: safer (no class is asserted that wasn't actually
decided by a person), and there is no live production user base yet
making the manual step costly.

**Not code** — this is an operational/data decision, not something to
implement as a migration default. Documented as a runbook:
`docs/runbooks/user-class-migration.md`, covering: what existing
`is_manager = true` users experience immediately after this deploys
(no capability lost, no capability gained until reclassified), the
step-by-step `curl`/API sequence for an Admin to list, decide, and
assign class + parent per user, and an explicit note that this runbook
does not itself decide when `is_manager` gets dropped — that is
deferred to a later migration once every affected user is reclassified.

---

## Staff Profiles — Complete

Requested 2026-08-13: "a full detailed profile page for each Staff
member... shows info publicly and access for modifications for admin,"
followed by "capability to import or export staff list profiles by
admin access as a batch for an organization."

### Confirmed spec

**Visibility (public/gated split):**
- Basic identity (full name, photo, job title, contact info) and
  organizational info (department, class label, reporting line,
  team memberships) — public to every authenticated user in the
  organization.
- Work stats (assigned/completed item counts, target summary) —
  visible **only** to `UserClass::TopLevelManager` and Admin.

**Editing:** Admin-only, every field, no exceptions — a separate
admin-only editing interface, not inline on the public view.

**Batch import/export:**
- Formats: CSV and JSON, both directions.
- Mandatory row fields: `user_id`, `full_name`, `contact_email`,
  `contact_phone`, `job_title`, `department` — a row missing any of
  these is rejected.
- `user_id` must reference an **already-existing account** — import
  creates/updates profiles, never accounts.
- Upsert semantics: a row whose `user_id` matches an existing profile
  updates it; an unmatched `user_id` creates a new profile.
- Row-level failures are skipped, not batch-fatal — the rest of the
  batch still imports, and every row's outcome (created/updated/failed
  + reason) is reported back.
- Admin-only, no exceptions.

**Explicitly deferred (confirmed by the person before building):**
work-stats section has no real data source yet — `work-domain::Task`
has no assignee field, and the Todo/Target domain that would populate
this
(`IMPLEMENTATION_PLAN_User_Hierarchy.md` Phase D) doesn't exist. Rather
than fake numbers or block the whole profile feature, `WorkStats` is
built with an explicit `unavailable()` state distinct from a real zero
— see `profile_domain::value::WorkStats`'s doc comment.

### New crate: `profile-domain`

`StaffProfile` aggregate (`BasicIdentity`, `OrganizationalInfo`,
`WorkStats` sections), following the exact `AggregateRoot` pattern
every other domain crate in this workspace uses — `create()`/`decide()`/
`apply()`, full inline test coverage, `#![deny(missing_docs)]`.
Deliberately does not depend on `security-application` or `file-domain`
(domain crates stay independent, §5.2) — `photo_blob_key` is a plain
string key the composing application resolves, and `class_label`/
`parent_display_name` are denormalized display copies with an explicit
documented sync obligation, not live joins.

Admin-only editing is enforced at the same `context.authority` seam
every other domain uses — real per-role enforcement
(`UserClass`/`is_admin` checks specifically) is the composing
application's job, matching this workspace's established precedent.

**9/9 tests pass, clippy clean.**

### `api-server` wiring

New `profile_repo: Arc<dyn Repository>` on `ApiState`, constructed the
same way as `notification_repo`/`approval_repo` (both Postgres and
SQLite branches). New route module `routes::profiles` (+ nested
`routes::profiles::batch`):

- `GET /api/profiles` — list all profiles in the caller's org, public
  fields only.
- `GET /api/profiles/:owner_id` — single profile. Returns one of two
  DTOs (`PublicProfileDto` or `ProfileDtoWithStats`) depending on
  whether `require_class(&[UserClass::TopLevelManager])` passes for the
  caller — the gate is a **type-level split**, not a field conditionally
  nulled out, specifically to avoid leaking "stats exist but you can't
  see them" vs. "no stats recorded" as distinguishable outcomes to an
  unauthorized viewer.
- `PUT /api/admin/profiles` — admin-only single upsert.
- `POST /api/admin/profiles/import` — admin-only batch import
  (multipart `file` field, CSV or JSON by content-type, falls back to
  CSV). Returns an `ImportSummary` with a per-row `RowResult`
  (`Created`/`Updated`/`Failed { reason }`), in input order.
- `GET /api/admin/profiles/export?format=csv|json` — admin-only
  whole-org export.

`routes::admin::require_admin` and `require_class` were widened from
private to `pub(super)` so `routes::profiles` could reuse them rather
than duplicating auth logic — `require_class` gained its first real
caller here (previously `#[allow(dead_code)]`, added ahead of its first
use during Phase A).

**No `Repository::load`-by-field capability exists** (that trait only
supports load-by-id) — `find_by_owner`/whole-org listing instead scan
the shared `aggregates` projection table directly via raw SQL, filtered
by `aggregate_type = 'staff_profile'`, reusing the same table/pattern
`query_handler.rs`'s own list-style queries already use, rather than
inventing a second query mechanism.

**A real bug found and fixed via testing, not just reasoning about the
code:** the first version of `find_by_owner` compared
`state["owner_id"].as_str()` against a UUID string and always got
`None` — `ObjectId(pub [u8; 16])`'s derived `Serialize` produces a JSON
array of 16 numbers, not a string (confirmed by reading
`platform_kernel::identifiers` directly, not assumed). This meant every
upsert silently created a duplicate profile instead of updating the
existing one. Caught immediately by
`upsert_creates_then_updates_a_profile`'s assertion that exactly one
profile exists per owner after two upserts — it failed with `left: 2,
right: 1` on the first real test run. Fixed by comparing byte arrays
instead of strings; regression-tested by the same test now passing.

**Verified end-to-end** with 6 new HTTP integration tests
(`tests/staff_profile_routes.rs`, same "compiling is not evidence"
philosophy as this project's existing test suites): upsert-creates-
then-updates (the test that caught the bug above), public view
correctness, admin-required enforcement on both the single-upsert and
import routes, JSON import with mixed success/failure rows (including
asserting the exact created/updated/failed counts), and a full CSV
import → export round-trip confirming `team_memberships`'
`;`-delimited encoding survives the round trip intact.

**Verification:**
```
cargo build --package profile-domain                          # clean
cargo test --package profile-domain                            # 9 tests, 0 failures
cargo clippy --package profile-domain --all-targets -- -D warnings   # clean
cargo build --package api-server                                # clean
cargo test --package api-server                                 # 16 tests, 0 failures (10 prior + 6 new profile routes)
cargo clippy --package api-server --all-targets -- -D warnings  # clean
```

**Not yet done:** `client-composition`/desktop wiring (this went
through `api-server` directly, since import/export is an Admin/API-
driven operation, not something desktop needs a separate
implementation of — `profile-domain` itself is client-composition-
ready whenever a Desktop UI is wanted, following the exact same wiring
pattern already used for `policy-domain`). No UI (Desktop or Web) for
viewing/editing/importing/exporting profiles yet. Work-stats population
blocked on `IMPLEMENTATION_PLAN_User_Hierarchy.md` Phase D, as
documented above.

---

## Admin Platform (`admin-shell`) — In Progress

Requested 2026-08-14: "separate the Admin platform — login, full
access, changes go through all platforms via the server."

### Confirmed decisions
- **Form:** a separate Tauri desktop app, distinct from `desktop-shell`.
- **Architecture:** **thin HTTP client** — talks directly to
  `api-server` over the network, no local SQLite, no sync engine, no
  embedded `client-composition`. This is what makes "changes go through
  all platforms via the server" true by construction: the app has no
  local state that could diverge.
- **Scope:** (1) user management, (2) Policy/Settings, (3) staff profile
  admin edit + batch import/export. Confirmed complete for now.
- **Admin capability is to be REMOVED from the existing Desktop/Web UI**
  once this app works — deliberately deferred until then, so the
  capability is never absent from both places at once.

### Done and verified
- `admin-shell` Rust/Tauri crate — scaffolded, registered in the
  workspace, `cargo check` clean. Deliberately minimal (~40 lines):
  no `AppState`, no DB pool, no Tauri command surface for
  domain commands, since the webview calls `api-server` over HTTP
  directly. Only native capability added is a file-picker plugin, for
  the batch profile import flow.
- `/api/auth/login` extended with `is_admin` and `class` (additive —
  `web-ui`/`desktop-shell` ignore fields they don't read), so the Admin
  platform can show "you don't have admin access" immediately after
  login rather than only via a 403 on the first action. All 16
  pre-existing api-server tests still pass.
- **Policy/LegalHold command + query support added to `api-server`** —
  see "the two-registry gap" below.
- `admin-ui` React app scaffolded: Tauri-matching Vite/TS config (port
  5174 so both desktop apps can run dev servers simultaneously), auth
  store/HTTP client/command+query wrappers ported from `web-ui`'s
  established working patterns, `App.tsx` with a two-layer gate
  (authenticated → is_admin), `Login.tsx`, `MainLayout.tsx`,
  `Users.tsx` (list/create/class/parent/activate-deactivate, verified
  against real route paths and handler return types).

### The two-registry gap — found and closed for Policy
`api-server` and `client-composition` maintain **separate** command/
query registries. Policy/LegalHold had been wired into
`client-composition` (which `desktop-shell` embeds) in an earlier
session, but `api-server` supported only `notification.Acknowledge`,
`approval.Approve`, `approval.Reject`. A thin HTTP client therefore
could not administer Policy at all — discovered before porting the
Settings UI, not after.

Added to `api-server`: `policy.CreatePolicyVersion`,
`policy.PublishPolicyVersion`, `policy.EvaluatePolicy`,
`policy.RegisterViolation`, `policy.RetirePolicy`,
`legal_hold.ReleaseLegalHold`, plus `policy.list/detail` and
`legal_hold.list/detail` on the query side, plus `policy_repo` and
`legal_hold_repo` on `ApiState` (both Postgres and SQLite branches).

**Deliberately NOT added:** `CreatePolicy` and `ApplyLegalHold`. Both
are *creation* commands, routed through `Aggregate::create()` rather
than `decide()`, so they cannot go through `handle_command` the way
every other arm in that match does. They need a separate creation path
(like `routes::profiles`'s own upsert handler) — flagged here rather
than faked.

**A real limitation, documented at the call site:** the dispatch block's
error type is `CommandError`, which cannot express an HTTP status, so
payload validation cannot happen inside it. `policy.CreatePolicyVersion`'s
structurally-typed `rules` payload is therefore parsed and validated
*before* the block, returning a real 400 on malformed input. Every other
supported command reads plain strings with `unwrap_or_default`.

**Verification:**
```
cargo check --package admin-shell                               # clean
cargo build --package api-server                                # clean, no warnings
cargo test --package api-server                                 # 16 tests, 0 failures
cargo clippy --package api-server --all-targets -- -D warnings  # clean
```

### Not yet done
- `Settings.tsx` (Policy/LegalHold UI) — backend is now ready, the page
  itself is not written. Porting from `desktop-shell`'s 706-line version
  requires converting every `Id16` (`number[]`, the raw `ObjectId` serde
  form Tauri IPC uses) to a plain UUID string, which is what
  `api-server`'s HTTP routes speak. Creation flows will also need the
  separate `CreatePolicy`/`ApplyLegalHold` path noted above.
- `Profiles.tsx` — not started. Backend fully ready and tested.
- `npm install` / `tsc` never run on `admin-ui` — **nothing in the React
  app has been type-checked or built yet.**
- Removing admin screens from `desktop-shell` — correctly deferred.

---

## Admin Platform — Completed 2026-08-15

Continuation of the previous session's work. All remaining pieces built
and verified.

### Systemic bug found and fixed: aggregate ids over `/api/query`
Every domain aggregate's id type is a single-field tuple struct
wrapping `ObjectId([u8; 16])`. Confirmed empirically (a standalone
repro, not assumed) that serde's default newtype serialization makes
this **transparent** — `id` reaches the client as a flat 16-number
array, not a UUID string. This affected **every** aggregate type
`/api/query` serves — Mission, Task, Notification, Approval, Report,
and now Policy/LegalHold — not just the new work. Confirmed `web-ui`
genuinely depends on `.id` being usable (used as React `key`s and as a
filter value for `timeline.list`'s `subject_id`), so this was a real,
live bug, not a hypothetical.

Fixed in `query_handler::normalize_public_state`: detects a 16-element
array of small integers at `id` and converts it to the equivalent UUID
string, structurally (shape-based), not per-aggregate-type — fixes
every affected type in one place. Also fixed a related gap:
`policy.detail`/`legal_hold.detail` were missing from the `is_detail`
truncation list, so they would have returned the whole org's policies
instead of one.

**Verified with 4 new tests** (`tests/query_id_normalization.rs`):
direct unit tests of the byte-array-to-UUID conversion (correct
conversion, non-array left alone, wrong-length array left alone) plus
one end-to-end HTTP test proving `policy.list` executes cleanly through
the fixed code path. Full `api-server` suite re-run after the fix:
**22/22 tests pass, 0 regressions** — the highest-risk verification in
this session, since the fix touches every aggregate type's read path.

### `admin-ui` — complete, type-checked
- `Login.tsx`, `MainLayout.tsx`, `Users.tsx`, `Profiles.tsx` — all
  present and correct (some pre-existed from earlier work in this
  session that had gone further than tracked; verified rather than
  blindly overwritten).
- `Settings.tsx` — written this pass: full Policy/LegalHold admin UI,
  ported from `desktop-shell`'s version with every `Id16`
  (`number[]`)-typed id converted to a plain UUID string, `useCommand`/
  `useQuery` swapped for their HTTP-backed equivalents, and
  `CreatePolicy`/`ApplyLegalHold` routed through the dedicated REST
  endpoints (`/api/admin/policies`, `/api/admin/legal-holds`) rather
  than `/api/command`, since those are `create()`-routed commands
  `handle_command` cannot dispatch.
- `npx tsc -b`: **zero errors** across the whole app — first real
  type-check of this codebase, not previously run.

### Remaining, explicitly deferred
- Removing admin screens from `desktop-shell`/`web-ui` — still
  correctly deferred until the Admin platform is used and confirmed
  working, per the original product decision.

### `admin-shell` full binary build — verified 2026-08-15
Previously only `cargo check`-verified due to sandbox disk exhaustion
mid-build on this crate's heavy GTK/WebKit2GTK/Tauri dependency chain.
Re-attempted after clearing 7.5GB of reclaimable disk space (stale
`target/` build cache, a leftover scratch debugging project, stray test
databases from this session's test runs). **Result: `cargo build
--package admin-shell` completes successfully** — produces a real,
executable ELF binary (`target/debug/admin-shell`, 224MB, confirmed via
`file`). `cargo clippy --package admin-shell -- -D warnings`: clean.
This closes the P0 item from the 2026-08-15 status report.
- `admin-shell`'s full Tauri/GTK build (icons, bundling, actual
  `cargo build` of the binary target rather than `cargo check`) not
  re-verified in this pass due to sandbox disk constraints; `cargo
  check` passed earlier in the session and no admin-shell source
  changed since, so this is a low-risk gap, but a real `cargo build
  --package admin-shell` (or `tauri build`) should be run in an
  environment with headroom before shipping.

---

## Allfather — resolved as narrow provisioning capability (2026-08-15)

Purpose clarified: ONYX will be sold to multiple customer organizations.
Reference-checked against a well-regarded multi-tenant SaaS pattern
(NestJS multi-tenant starter, via Context7) before deciding — confirmed
that pattern has **no standing cross-tenant super-admin account** at all.
Per-tenant config (feature flags/settings, ONYX's equivalent being
Policy) is done by that tenant's own OWNER/Admin, scoped to their own
org — not by a vendor-side account reaching across tenants. Tenant
creation there is a plain authenticated action, not a special role.

Decision: **drop the standing "Allfather account" design entirely.**
Replaced with a narrow, one-off provisioning action:
- Creates a new organization + its first Admin account, then stops.
- No ongoing/standing access into that org afterward — the new org's own
  Admin takes over immediately, same as any other org.
- Invoked by the vendor (you) specifically, not a grantable role, at
  least for now.
- Because it's a rare, one-off action rather than a daily-use account,
  it does not need the heavy security apparatus (MFA policy, persistent
  session management, unforgeable audit trail, kill switch) that a
  standing cross-org account would have required — that entire line of
  design work (previously flagged as a needed 6th requirement) is now
  unnecessary, because the account that would have needed it no longer
  exists.

Open, deliberately not decided yet: how the first Admin's credential is
handed to the customer (temporary password vs. an invite/email flow).
Small, does not block building the provisioning action itself.

Status: **designed, not yet built.** Small in scope — a single
provisioning route/action plus reusing the existing org+Admin creation
logic already proven out in `routes::admin`'s bootstrap path. No longer
a P1 blocker; ready to schedule as ordinary implementation work.

---

## Phase C/D — Staff loan mechanics and Todo/Target verification details — resolved 2026-08-16

All previously-open questions in `todo-domain` requirements gathering
(Phase C staff loans, Phase D Todo/Target verification) resolved this
session, in the person's own words, one at a time per their standing
instruction to never assume or guess:

**Target hit/miss determination:** "Number two, I believe, is more
reasonable" — judged at verification time by the parent Manager, same
shape as Todo's flawless/with-deficiencies decision. No live tracking,
no interim "achieved" flag, no `TargetOutcomeRecorded` event. Also
confirmed binary, not incremental: "either hitting the target or
missing the target... cannot partially hit the target."

**Verification comment field:** "It's not like a requirement. More like
a checkbox. If needs description or comment, you can hit the checkbox
and add a comment." Always optional, independent of outcome — applies
identically to `TodoList` and `TargetList`. Corrects the earlier
tentative `VerifiedWithDeficiencies { comment: String }` shape (which
implied the comment was mandatory for that outcome) to a single
`Option<String>` regardless of outcome.

**Staff loan duration:** "Yes, they must be fixed at duration time" —
fixed start and end date-time, set when the loan is created, not a
rolling "start + N days."

**Staff loan extend/end authority — three distinct gates, not one:**
- Starting a loan: real owner approves (previously confirmed,
  unchanged).
- Extending an active loan: "for extension, the employee itself need to
  approve that" — the staff member's own approval, not either manager's.
- Ending a loan early: "Canceling alone is a normal thing. Nobody would
  be affected negatively by that. Everybody going back to their work is
  a normal thing" — no approval needed from anyone; either manager may
  end it unilaterally.

**Staff loan notifications and expiry mechanism:** both managers and
the staff member are notified 2-3 days before a loan's end date (advance
warning) and again when it actually ends, with the option to extend or
let it lapse at that point. Since advance warning requires proactively
noticing an upcoming date rather than reacting to a request, this
settles the previously-open "background job vs. on-read check" question
in favor of **a scheduled background job** — an on-read check has no
natural moment at which to fire a warning 2-3 days ahead of time.

**Status:** all Phase C/D product questions are now resolved. Full
detail recorded in `DESIGN_User_Hierarchy_Chain_of_Authority.md` §2.1,
§4.0.1.1, §4.0.2, and open-questions items 9-12; corresponding build
guidance in `IMPLEMENTATION_PLAN_User_Hierarchy.md` Phase C (C.1-C.4)
and Phase D (D.2-D.3). Implementation of `todo-domain` may now proceed
— no code existed for this feature prior to this session's resolutions,
per the person's standing "never assume, never guess" instruction.

---

## Phase C/D implementation — built, wired, and live-tested — 2026-08-16

Following the resolution of all Phase C/D product questions (previous
entry), the full backend was built end-to-end this session, in
dependency order, with live verification wherever the sandbox allowed
it (no Postgres available — see the explicit caveat below).

### What was discovered already built (not built this session)

**Phase A (User Hierarchy data model) was already fully implemented**
in an earlier, uncatalogued session: `UserClass` enum, `parent_user_id`
tree column with cycle-detection, both Postgres/SQLite `UserStore`
adapters, and the `require_class` permission helper — all present,
tested (24/24 passing), and confirmed working via a fresh test run this
session. This was a genuine discovery partway through this session, not
assumed — it changed the build order, since Phase A no longer needed
building, only verifying.

**Persistence is fully generic** — one polymorphic `Repository` over
`serde_json::Value`, keyed by an `aggregate_type` column on a single
shared `aggregates` table. `todo_list`/`target_list`/`staff_loan`
needed no new migration to be persisted — they slot into the existing
table exactly as `policy`/`legal_hold` already do.

### What was built this session

1. **`api-server` HTTP wiring** — `ApiState` gained
   `todo_list_repo`/`target_list_repo`/`staff_loan_repo`; `/api/command`
   gained full dispatch for every `todo_list.*`/`target_list.*`/
   `staff_loan.*` command; `/api/query` gained `todo_list.list/.detail`,
   `target_list.list/.detail`, `staff_loan.list/.detail`; a new
   `routes::todo_admin` module provides the three `create()`-routed
   REST endpoints (`POST /api/todo/lists`, `/api/todo/targets`,
   `/api/todo/staff-loans`), mirroring `routes::policy_admin`'s
   established precedent exactly. **Live-tested**: full create → submit
   → verify → re-query round trip against a real running server (SQLite
   backend), plus staff-loan request → approve.

2. **A real bug found and fixed via that live test**: JWT scope's
   `command_types` allow-list (a second, separate authorization gate
   from `routes::command`'s own dispatch match) needed every new
   command type added explicitly, or every request 403'd with
   `COMMAND_NOT_AUTHORIZED` regardless of routing being correct. Fixed
   in `issue_token` (`routes/mod.rs`) — same class of gap the
   Policy/LegalHold integration hit earlier, per that code's own
   pre-existing comment.

3. **C.3 — the staff-loan background job.** New
   `worker::staff_loan_scheduler` module (mirrors
   `scheduler_loop::scheduler_tick_postgres`'s exact shape) scans for
   loans within the 2.5-day advance-warning window (midpoint of the
   confirmed "2 or 3 days" range) and loans past their `end_at`,
   enqueuing `StaffLoanAdvanceWarning`/`StaffLoanExpiry` jobs. Two new
   handlers in `job_runner.rs` execute them: insert one `notification`
   row per recipient (staff member, real owner, borrowing manager, per
   design doc §2.1's confirmed three-party notification), and (for
   expiry) transition the loan to `Expired` via direct SQL, matching
   `execute_timeline_trigger`'s established precedent of mutating
   `aggregates.state` directly for scheduled transitions rather than
   round-tripping through the domain crate's in-process types.
   **Verified**: compiles clean, clippy clean, and the exact JSON field
   names/shapes the SQL depends on were confirmed against
   `todo_domain::StaffLoan`'s real serialized output via a throwaway
   test (`status: "Requested"`, `window: {start_at, end_at}` as bare
   integers — both bare-string/bare-number shapes as expected from
   plain externally-tagged serde). **Explicit caveat, not glossed
   over**: the raw SQL itself (`jsonb_set`, `->>`/`->` JSON path
   operators) was never run against a live Postgres — none was
   available in this sandbox, and disk space (as low as 372MB free at
   points this session) made installing one unsafe to attempt. The
   `jsonb_set`/`create_missing` semantics were checked against
   PostgreSQL's own documentation, which is a real but weaker form of
   verification than actually running it. This is flagged here plainly
   so it is not mistaken for the same level of confidence as the
   live-tested HTTP paths.

4. **D.4 — verifier-resolution.** New `verifier_resolution` module in
   `api-server`, combining Phase A's tree (`UserStore.parent_user_id`)
   with Phase C's active-loan widening
   (`StaffLoan::grants_verification_authority_to`, confirmed as the one
   piece of this logic that already lived on the aggregate itself).
   Wired into `/api/command`'s dispatch: `VerifyTodoList`/
   `RejectTodoList`/`EscalateTodoList` (and the `TargetList`
   equivalents) now load the aggregate's `owner`, resolve authorized
   verifiers, and reject unauthorized callers before dispatch.
   Deliberately does **not** include Phase E's escalation widening —
   Phase E has no routing/target-selection code yet, so a stub there
   would misleadingly look handled when it silently does nothing; the
   module's docs say so explicitly. **Live-tested, both directions**:
   an unrelated third user was correctly rejected
   ("actor is not an authorized verifier for this list's owner"); the
   real tree-parent Manager was correctly accepted and the verification
   succeeded.

5. **D.5 — Team Leader pre-check visibility redaction.** While building
   this, found and fixed a **real pre-existing bug**: `TodoList`'s and
   `TargetList`'s `apply()` methods discarded the
   `TeamLeaderPreCheckRecorded` event's `notes`/`checked_by`/
   `checked_at` entirely (a `{ .. }` match pattern), so the pre-check's
   substance never reached the aggregate's stored state at all — there
   was nothing for redaction to redact. Fixed by adding a
   `TeamLeaderPreCheck` value struct and a
   `team_leader_pre_check: Option<TeamLeaderPreCheck>` field to both
   aggregates, with two new regression tests
   (`team_leader_pre_check_then_verify_succeeds`'s strengthened
   assertions, `target_list_team_leader_pre_check_is_stored`) confirming
   the data is now genuinely stored. With that fixed, added
   `redact_team_leader_pre_check_for_viewer` to `query_handler.rs`:
   removes `notes` specifically when the querying viewer equals the
   list's `owner` (Staff viewing their own list), leaving
   `checked_by`/`checked_at` intact — matching design doc §2.2's
   "existence visible, substance never" rule precisely.
   **Live-tested, both directions**: the Staff owner's query result for
   `team_leader_pre_check` showed `checked_at`/`checked_by` but no
   `notes` key at all; the same query as the Team Leader who performed
   the check showed the full record including `notes` in full.

### Explicit scope boundary — what remains unbuilt

Per the person's own choice this session ("Continue with D.4 + D.5
(backend logic only, no UI)"): no UI work was done for any of this.
`desktop-shell`, `admin-shell`, and `web-ui` have no todo/target/loan
screens. Also unbuilt, unchanged from `todo_domain::lib`'s own
documented scope: Phase E's escalation routing/target-selection logic
(only the recording commands exist), and any Postgres-specific runtime
verification of the C.3 job (see caveat above).

---

## Live PostgreSQL verification of C.3 (staff-loan background job) — closed 2026-08-16

Following the previous entry's explicit caveat (C.3's raw SQL never run
against a live Postgres, no Postgres available in the build sandbox),
the code was handed off to a Manus AI agent with access to a real
Postgres instance, via a git bundle (`onyx-todo-domain.bundle`) built
on the same commit lineage.

**Result: verified, and a real bug was found and fixed.**

Manus provisioned PostgreSQL 16.14, applied the repository's migrations,
and wrote a new live integration test —
`worker::staff_loan_scheduler::postgres_integration_tests::staff_loan_warning_and_expiry_round_trip_on_postgres`
— that inserts a real `staff_loan` row and drives both the
advance-warning and expiry paths end to end against the database,
asserting on actual persisted state rather than mocked behavior. The
test is a documented no-op when `DATABASE_URL` is unset or
non-Postgres (confirmed by re-running `cargo test --package worker`
without `DATABASE_URL` set: the test completes in ~0ms, not skipped
silently but printing why).

**Bug found**: `job_runner.rs`'s notification-insertion loop computed
each recipient's user id (staff member, real owner, borrowing manager)
correctly via `staff_loan_recipients()`, but then discarded it with
`let _ = recipient_id;` before writing the notification row — so all
three notifications were inserted with no field identifying who they
were addressed to. This was a real defect in the original
implementation, not a testing artifact.

**Fix**: added `recipient_id: Option<String>` to
`api_server::routes::command::NotificationAggregate`, marked
`#[serde(default)]` so existing/legacy notifications without the field
still deserialize correctly, and threaded the real recipient id through
`insert_notification()` in both the advance-warning and expiry
handlers. The new Postgres test asserts the exact set of three
recipient ids (via `BTreeSet` comparison against the staff member's,
real owner's, and borrowing manager's actual ids) for both
notification types, not just a count — so a regression here would be
caught, not just "3 rows exist."

**Re-verified in this sandbox** after pulling Manus's changes back in:
`cargo check --package worker`, `cargo clippy --package worker -- -D
warnings`, and `cargo check`/`clippy`/`test --package api-server` (22
tests, matching Manus's reported count) all pass clean from a cold
build cache. `cargo test --package worker` without `DATABASE_URL` set
correctly no-ops the new test rather than failing.

**Status**: the C.3 background job is now fully verified — compiled,
clippy-clean, unit-tested (`todo-domain`'s 42 tests covering the domain
logic it depends on), and live-tested against a real PostgreSQL
instance for both its scan and execution paths. See
`IMPLEMENTATION_PLAN_User_Hierarchy.md` §11.3 for the updated technical
detail (the original caveat text is kept, collapsed, for the historical
record rather than deleted).

**Remaining, unrelated to this feature**: the workspace's
Docker-backed Testcontainers `e2e` test package could not be run in
either sandbox (no Docker daemon/socket available in this one, nor in
Manus's). This is a pre-existing gap in the broader workspace's test
infrastructure, not something this feature introduced or is
responsible for closing.

---

## First UI slice: Todo/Target lists in web-ui — 2026-08-16

Following the backend work and its live-Postgres verification (previous
two entries), a first UI slice was built for Todo/Target lists —
create, submit, verify/reject/escalate — closing part of the "no UI
work" gap flagged in the status report.

**Home: `web-ui`, not `admin-shell`.** Confirmed by reading
`admin-shell`'s `App.tsx` before writing anything: its entire route
tree sits behind `AdminOnlyLayout`, which requires `is_admin`. Design
doc §4.0.1 confirms Todo/Target creation is bidirectional (Staff or
Manager) — an Admin-only home would incorrectly exclude the people this
feature is actually for. `web-ui` has no such gate, only authentication,
and already talks to `api-server` over the same HTTP surface this
session built against.

**Scope of this slice**: self-authored creation only (the current user
creates a list/target for themselves), submit, and the three
verifier-gated actions (verify with outcome + optional comment, reject
with reason, escalate with reason). Assigning a list to a *different*
owner (`ManagerAssigned` origin) needs a user picker this UI doesn't
have yet — deliberately deferred as a follow-up, not a corner cut.
Staff Loans has no UI yet.

**A real regression found and fixed during this work**: an early draft
added `'draft'` to `StatusBadge`'s `'info'` tone bucket. A pre-existing
frozen wire-contract test (`tests/unit/contracts.test.ts`) locks
`statusTone('draft')` to `'neutral'` — this broke it. Caught by
actually running `npx vitest run`, not assumed correct from the diff
alone; fixed by leaving `'draft'` out of every explicit bucket so it
falls through to the existing `'neutral'` default, which was already
correct.

**Also caught and corrected before that**: an initial draft of the new
page components invented several CSS classes that don't exist in this
codebase (`tab-toggle`, `button-active`, `card-list-compact`) and one
fabricated external link. Caught by grepping `styles.css` directly and
reading working precedent files (`Missions/MissionList.tsx`,
`Missions/MissionDetail.tsx`, `Approvals/index.tsx`,
`admin-shell`'s `Settings.tsx`) rather than trusting the first draft —
rewritten to use only classes confirmed present in the stylesheet.

**Verified**: `npm install` (542 packages), `npx tsc -b` (zero type
errors), `npx vite build` (515 modules, production build succeeds),
`npx vitest run` (130 tests passed, 7 skipped — pre-existing
`e2e/real-server.test.ts` tests that need a live backend, unrelated to
this change).

**Not verified**: ESLint. This checkout has no `eslint.config.*` or
`.eslintrc*` file anywhere in the repository — confirmed by search, a
pre-existing gap in the checkout, not something this change introduced
or attempted to paper over by inventing a config.

**Files**: `web-ui/src/pages/TodoTargets/` (new: `index.tsx`,
`CreateListForm.tsx`, `ListCard.tsx`, `ListDetail.tsx`,
`DecisionDialog.tsx`); extended `types/query.ts`, `types/command.ts`,
`hooks/useCommand.ts`, `components/StatusBadge/index.tsx`; registered
in `App.tsx` (route `/todos`) and `components/Layout/Sidebar.tsx` (nav
link "Todos & Targets").

---

## Two remaining product decisions resolved — 2026-08-16

**B.4 — first Admin credential delivery: invite email with a setup
link.** Not a temporary password shown once at creation time. No code
built yet for this — it's a detail of Phase B.1–B.2's provisioning
action, which itself hasn't been built. Recorded so the eventual
implementation doesn't need to re-ask this question.

**Admin-screen removal from `desktop-shell`/`web-ui`: confirmed, go
ahead.** This had been deliberately held since the Admin Platform
(`admin-shell`) was first built, pending confirmation it works in
practice. That precondition has since been met repeatedly this session
— live-tested, 25 passing `api-server` tests, real HTTP round trips —
so the person confirmed removal should proceed now. See the next
commit for the actual removal.

---

## Staff-loan approval authorization gap — closed via Manus verification — 2026-08-17

Following the previous handoff's explicit gap ("staff_loan.* commands
have no real server-side authorization check"), the code was handed to
Manus with a tightly-scoped prompt covering exactly that gap plus one
other item (the `web-ui` "Involving me" filter, against the
already-fixed id-normalization). Both were confirmed and closed.

**Confirmed real, then fixed**: an unrelated authenticated user could
send `ApproveStaffLoan` over the real HTTP API and receive HTTP 200 —
reproduced first, then fixed, not assumed. A new
`require_staff_loan_authority` helper in `routes/command.rs` loads the
persisted `StaffLoan` before dispatch and enforces design doc §2.1's
three approval gates per command:

- `ApproveStaffLoan`/`DeclineStaffLoan`/`EscalateStaffLoan`: the
  current decision-maker — the real owner, or the loan's
  `escalated_to` once escalated — via
  `StaffLoan::grants_approval_authority_to()`, which already existed
  from the escalation work but had never actually been called from the
  HTTP dispatch path.
- `ExtendStaffLoan`: the staff member being loaned, and only them.
- `EndStaffLoanEarly`: the real owner or the borrowing manager, no
  approval required (per design doc §2.1 — "canceling alone is a
  normal thing").
- `ExpireStaffLoan`: rejected outright from `/api/command` — this
  transition is worker-only (the scheduled background job calls it
  directly against the database), no end-user token is ever a valid
  authority for it.

Denial responses use the existing `"not permitted"` message
convention so the command-error mapper returns HTTP 403, matching
every other authorization rejection in this codebase rather than the
generic HTTP 400 a plain domain-rule violation gets.

**New test coverage**:
`crates/bins/api-server/tests/staff_loan_authorization.rs` — 3 real
HTTP tests against a real spawned server: an unrelated user rejected
for both approval and decline, the real owner accepted, and the
escalation target accepted after the real owner escalates (correctly
supplying the incremented `expected_authority_epoch` on the follow-up
approval, since escalation advances it).

**Re-verified in this sandbox** after pulling the fix back in:
`cargo check`/`clippy --tests` clean, the new test file's 3 tests pass,
and the full `api-server` suite (28 tests total, matching Manus's
reported count) passes with no regressions.

**Also confirmed in the same handoff**: the `web-ui` "Involving me"
filter on `StaffLoansPage` (written earlier this session against what
turned out to be a broken wire shape, then indirectly fixed by the
generic id-normalization change) was verified to actually work now,
via a new page-level integration test
(`web-ui/tests/integration/staff_loans_filter.test.tsx`) that renders
the real page through its production query hook and confirms the
correct three loans remain selected after toggling the filter. Full
`web-ui` suite: 131 passed, 7 skipped (pre-existing live-backend
tests), matching the expected count. **Note**: only the Rust source
files (`command.rs`, `staff_loan_authorization.rs`) were transferred
back into this sandbox and re-verified directly; the `web-ui` test
file itself was not transferred, so its presence/passing is reported
here on Manus's word, not independently re-confirmed in this sandbox.

**Status**: both items from the second handoff are closed. The
StaffLoan/escalation feature set built this session is now fully
verified — domain logic, HTTP wiring, D.4/D.5, escalation routing and
widening for both Todo/Target and staff loans, the C.3 background job
(live Postgres), field normalization, and now staff-loan command
authorization — with real tests behind every piece, not just compile
checks.

---

## Docker availability for the e2e suite — confirmed usable — 2026-08-17

Following the previous entry's honest "reported unavailable, not
independently re-checked" caveat, Manus was asked directly whether
Docker could be enabled in its sandbox at all, in any form, before
treating the earlier "no Docker daemon" report as a fixed limitation.

**Answer: yes, Docker is usable.** Not preinstalled or pre-started, but
the sandbox permitted a local Docker Engine install (`docker.io`,
Engine 29.1.3) and a manually-launched daemon. One real, sandbox-specific
snag came up and was resolved without touching the test harness: the
default daemon configuration tried to program a legacy iptables `raw`
table the sandbox doesn't support, which broke container networking;
starting `dockerd --iptables=false --ip6tables=false` fixed it. This is
a daemon configuration adaptation, not a code change — `test_harness.rs`
was not modified, and the earlier handoff's explicit instruction not to
patch the harness unless Docker was genuinely unavailable in every form
was correctly respected (it wasn't needed).

**The real `crates/team8-e2e-tests/tests/all_journeys.rs` suite was
then actually run** (`cargo test --package e2e --test all_journeys --
--nocapture`), from a full source compile, against real
Testcontainers-managed containers:

- `journey_1_mission_lifecycle` — passed
- `journey_2_task_workflow` — passed
- `journey_3_conflict_resolution` — passed
- `journey_4_approval_workflow` — passed
- `journey_5_notification_sync` — ignored (test's own message: "Team 5
  client event integration is not production-complete")
- `journey_6_p2p_sync` — ignored ("requires signed Team 5
  desktop/mobile clients and radio adapters")
- `journey_7_background_sync` — ignored ("requires Team 5 iOS
  BGTask and Android WorkManager release builds")

**4 passed, 0 failed, 3 ignored**, all in 10.67 seconds after
compilation. The three ignored journeys are ignored by the tests'
*own* declared reasons — genuine feature-incompleteness in other
teams' work (client/mobile), unrelated to Docker, unrelated to this
session's Todo/Target/StaffLoan work, and not something to chase down
here.

**Status**: this closes the "Docker/e2e suite could not be run"
item that had been carried as an open gap across every status report
and handoff since it was first noticed. It was a real environment gap
in the specific sandboxes used earlier, not a fixed or permanent
limitation of the workspace itself — confirmed by actually trying,
not assumed either way.

---

## Remaining web-ui workflow gaps — closed 2026-08-17

**Decision: ordinary authenticated users receive a reduced, active,
same-organization picker directory; `/api/admin/users` remains
admin-only.** A general user picker cannot call the administrative route
without either failing for Staff/Managers or broadening an admin
capability. The new `GET /api/users` route authenticates a normal bearer
token, filters `UserStore::list()` to active records in that token's
organization, and returns only `{ id, username }`. It never exposes
admin state, user class, reporting-line data, or account activation
metadata. A real HTTP test exercises the route as an ordinary user and
confirms the reduced response, inactive exclusion, tenant exclusion, and
401 behavior without a bearer token.

**Assignment and StaffLoan creation now use one shared picker.**
`web-ui/src/components/UserPicker.tsx` provides a small searchable
select backed by the new endpoint. Todo/Target creation retains
self-authored work and adds the explicit alternative to assign to a
picked colleague, correctly sending `ManagerAssigned` with that person
as `owner`. StaffLoan creation replaces all three raw UUID inputs with
staff-member, real-owner, and borrowing-manager selectors while retaining
the distinct-manager validation.

**Escalations now have actionable, user-focused visibility.** Todo and
Target lists include an **Escalated to you** view. Selecting a routed
item opens the existing action-capable detail panel, which now treats an
`Escalated` list as decidable only when the signed-in user is its
`escalated_to` target. Staff Loans add the matching filter and present
Approve/Decline to the current escalation target, replacing the real
owner only when an escalation target exists — the same rule the API
server enforces.

`web-ui/tests/integration/ui_gap_workflows.test.tsx` covers picker
search/selection, an escalated Todo decision reaching Verify/Reject/
Escalate controls, and an escalated StaffLoan reaching Approve/Decline.
These are production-page tests using the established MSW/query setup,
not isolated filter-only helpers.


---

## Native desktop notifications — complete 2026-08-17

**Decision: make notifications a shared domain capability, then compose the
same aggregate into the native client rather than creating a desktop-only
parallel model.** The notification aggregate, command, event, and error
were previously defined only inside `api-server::routes::command`, even
though `client-composition` is the desktop and mobile composition root.
A new `notification-domain` crate now owns `NotificationAggregate`,
`NotificationCommand`, `NotificationEvent`, and `NotificationError`.
`api-server` re-exports those types for route compatibility, while
`client-composition` depends on the same crate. This removes the
cross-client type boundary without changing the already-working API
command contract.

**Inbox and acknowledgement are locally composed, tenant- and
recipient-scoped.** `AppState` now registers a SQLite notification
repository, a `NotificationDecisionHandler` for `Acknowledge`, and
`ListNotificationsHandler` / `GetNotification` queries. The list handler
filters the local projection by both organization and recipient, so the
native client cannot use an unscoped aggregate scan as an inbox. A real
SQLite integration test drives a notification through the actual command
registry, confirms recipient filtering, persists acknowledgement, and
observes delivery through the actual event bus.

**Event delivery uses the existing outbox path, not new polling or push
infrastructure.** `api-server::command_handler::handle_command` now
registers each committed event with the local outbox before returning.
That makes the existing `SyncAgent::run_outbox_pump` publish committed
notification events to `EventBus`; the desktop shell starts
`SyncAgent::run()` at application startup; and its established
`subscribe_events` bridge forwards the event to the webview as
`onyx:event`. This corrects the missing local-outbox registration that
would otherwise have made event listeners inert despite successful command
persistence.

**Desktop UI scope:** `desktop-shell/ui/src/pages/Notifications.tsx` is a
native inbox routed at `/notifications` and exposed in the main sidebar.
It uses only the established Tauri `useQuery("ListNotifications")` and
`useCommand("Acknowledge")` paths, and refreshes on the existing
`onyx:event` stream. No browser HTTP request, axios/fetch client, polling
loop, or desktop-specific push subsystem was introduced. A manual Refresh
control remains a user action and recovery affordance, not background
polling.

**Verification:** `cargo fmt --all -- --check`; focused `cargo check` for
`notification-domain`, `client-composition`, `desktop-shell`, and
`api-server`; focused `cargo clippy --all-targets -- -D warnings`; and
`cargo test --package notification-domain --package client-composition`
all completed successfully. The latter ran 2 notification-domain tests
and 31 client-composition tests, including
`app_state_wires_notification_inbox_acknowledgement_and_events`. The
native UI completed `npx tsc -b` with zero errors and `npx vite build`
successfully (41 modules). No desktop-shell UI automated-test harness
exists in this workspace; none was invented for this scoped feature.

---

## Desktop app CI — build for Linux, macOS, and Windows — 2026-08-17

Confirmed a real gap by reading the actual workflow files, not
assuming: `ci.yml`'s `check` job compiled `desktop-shell` on Linux
only, via a plain `cargo build --workspace` — a compile-error check,
never invoking Tauri's bundler. `release.yml`'s existing 3-platform
matrix (`release-binaries`) only ever built the server-side binaries
(`api-server`/`worker`/`migration-tool`/`sync-agent`); `desktop-shell`
was never in that job's package list. Net result: no CI job anywhere
in this repo produced an installable desktop app, on any platform,
despite `tauri.conf.json` already declaring `"targets": "all"`.

**Fixed**: added `release-desktop` to `release.yml` — a 3-platform
matrix (`ubuntu-24.04`, `macos-14`, `windows-2022`, matching
`release-binaries`' own OS choices exactly) using
`tauri-apps/tauri-action` to actually invoke the bundler and produce
`.deb`/`.AppImage` (Linux), `.dmg` (macOS), and `.msi`/NSIS `.exe`
(Windows). Every Linux system dependency was taken directly from
`ci.yml`'s own already-proven install list, not re-derived. Every
non-obvious `tauri-action` input (`projectPath`, `args`, `tauriScript`,
and the "omit `tagName`/`releaseName`/`releaseId` to build only, no
release interaction" behavior) was checked against the action's own
documentation before use — this repo's `publish` job already owns
assembling the final GitHub release, so `tauri-action` must not also
try to create one.

**Two real mistakes were caught during review and fixed before
committing**, not left in: a job-level `working-directory` default
combined with a `.`-override step does not resolve to the repo root
the way it was first written (GitHub Actions resolves relative
`working-directory` settings relative to the already-set default, not
the workspace root); and the bundle-locating step's path plus the
macOS bundle list were both wrong on the first pass (this is a Cargo
workspace, so build output lands in the workspace-root `target/`, not
a per-crate one — and `.app` is a directory, not a file, so it can't
be found by a `-type f` search alongside `.dmg`). Both fixed before
this change was committed.

**Explicit, undecided gap, not silently assumed away**: real code
signing (Apple Developer ID + notarization for macOS, Azure Key Vault
or equivalent for Windows) needs secrets this repo does not yet have.
The macOS signing step is `continue-on-error: true` with Tauri's own
documented ad-hoc-signing fallback, so an unsigned build still
succeeds — but it will show a Gatekeeper warning on macOS and a
SmartScreen warning on Windows until real signing is configured. That
is separate follow-up work, not something to invent credentials for
here.

**Not run**: GitHub Actions cannot execute inside this build sandbox.
This was verified by careful manual review — cross-referencing every
non-obvious `tauri-action` input and Tauri's own CI documentation, and
catching the two real mistakes above during that review — not by an
actual CI run. The workflow only triggers on `push: tags: ["v*"]`
(unchanged), so it has not run automatically either.

## admin-shell: runtime-configurable server address (2026-08-19)

**Problem, found during live deployment, not in review**: `admin-shell`
(`crates/bins/admin-shell/ui/src/api/client.ts`) had the backend server
address hardcoded at build time — `VITE_API_BASE` env var, defaulting
to `http://127.0.0.1:3000` — with no way to change it after the app was
built. This meant every install of the Admin app, on every PC, could
only ever talk to a server running on that same machine. On a second
PC, trying to log in silently tried to reach `127.0.0.1:3000` on that
PC itself (nothing there), got a network error, and — because `Login.tsx`
previously caught every failure identically — displayed "Invalid
username or password," which is misleading: the request never reached
the real server at all. This surfaced directly during setup on real
hardware, not from a review pass.

**Fix**: the server address is now a runtime, user-editable setting,
persisted in `localStorage` (not `sessionStorage` — unlike the auth
session in `utils/auth.ts`, this must survive app restarts, which is
the entire point of making it configurable). New file
`utils/serverAddress.ts` holds the get/set/validate logic;
`api/client.ts`'s request interceptor now resolves the base URL fresh
on every request instead of once at module load, so a saved change
takes effect immediately with no app restart required.

**Two places to edit it, not one, and this was deliberate**: the full
version lives on the Settings page (`pages/Settings.tsx`,
`ServerConnectionSettings`), but Settings sits behind
`ProtectedLayout`'s login gate — someone whose app is still pointed at
the wrong address can't log in yet, so they'd never be able to reach
it to fix it. A second, collapsed "Server address / connection
settings" section was added directly to the Login page itself
(`pages/Login.tsx`, `ServerAddressField`), reachable pre-login, so the
address can be corrected without ever needing a successful login
first.

**"Test & Save" is one action, not two**: both copies of the field
call `GET /health` against the entered address before saving anything.
An unreachable address is never silently persisted — the person sees
"could not reach a server at this address" immediately, rather than
saving a bad value and only discovering it's wrong at the next failed
login. `Login.tsx`'s submit handler also now distinguishes a network
failure (`isNetworkError`, no `error.response` present — axios only
sets that when a response was actually received) from a real
401/credentials rejection, and only shows the server-unreachable
messaging (auto-expanding the address field) for the former.

**Verified**: `npx tsc -b` clean, `npx vite build` clean (95 modules,
no errors). No existing test files cover `admin-shell/ui` — nothing to
regress, none written for this change either since the app has no test
harness in place yet to add to consistently.

**Not done, explicit gap**: `staff_manual`/desktop-shell and mobile
clients were not touched — this fix was scoped to `admin-shell` only,
since that's the app that was actually failing on real hardware in
this session. A quick grep afterward found `desktop-shell/src/lib.rs`
has a similarly-shaped hardcoded fallback
(`ws://127.0.0.1:3000`, line ~353) — but it's Rust, not the TypeScript
`localStorage`/interceptor pattern used here, so it needs its own
investigation and fix, not an assumption that this change covers it.
`web-ui` showed no equivalent hardcoded address at all in the same
grep — unconfirmed why (possibly same-origin deployment), not
verified further. Neither is fixed by this change.

## desktop-shell: real per-user login, replacing random-org-per-launch (2026-08-19)

**The gap, found by reading the code, not assumed fixed**: `desktop-shell`
(the Staff native app) previously had NO login screen and NO real
session at all. Every launch called `OrganizationId::new_random()` —
flagged by its own `TODO(auth/org resolution)` comment as "no
login/auth flow exists yet." Worse, the frontend's `useSession.ts`
independently generated its own placeholder ids, which could disagree
with the Rust side's random id — meaning a command could target a
different `organization_id` than the one `AppState`'s aggregates were
actually scoped under. This was confirmed directly with the person
before any code was written, since it's a materially bigger gap than a
hardcoded address (see the conversation this session): the person
explicitly chose full per-user login (matching `admin-shell`) over a
lighter-weight device-pairing alternative that was offered.

**Design**: `desktop-shell` embeds a full local `AppState` (its own
SQLite, command/query registries, sync agent) rather than talking to
`api-server` per-request like `admin-shell` does — so login here does
two things, not one: (1) authenticate against `api-server`'s real
`POST /api/auth/login` (same endpoint `admin-shell` uses) to learn the
real `organization_id`; (2) persist that identity via the existing
`SecureStorage` port (`secure_storage/mod.rs` — its own doc comment
already named `"auth.refresh_token"` as an intended key, so this was
scaffolded for, never connected) so the *next* launch resumes the same
org instead of a fresh random one.

New file `session.rs`: `authenticate()` (calls `/api/auth/login`,
maps the response into a `StoredSession` that travels with its
`server_address` — reusing tokens against a different server would be
a real correctness bug, not just unlikely), `save`/`load`/`clear`
(one JSON blob under one storage key, atomic — no risk of a token
saved with a missing/mismatched org id after a partial write).

`lib.rs` changes: managed state is now `Arc<RwLock<Arc<AppState>>>`
(was a bare `Arc<AppState>`) so `login`/`logout` can build a *new*
`AppState` under the real org id and atomically swap it in without
restarting the Tauri process — verified against Tauri 2's own state
management docs as the supported pattern before choosing it over a
process-restart approach. Three new commands: `login`, `logout`,
`get_current_session` (lets the frontend decide on launch whether to
show the login screen or go straight in). `SessionInfo` (what the
frontend actually receives) deliberately omits the raw tokens — all
authenticated calls happen server-side in Rust, so there's no reason
for a webview-reachable value to hold them. The sync relay address is
now *derived* from the login server address
(`relay_ws_endpoint_from_http`), not a separately-configured
`ONYX_RELAY_ENDPOINT` — one address to ever enter, not two that could
silently drift apart. The device's local SQLite/`ReplicaId` are
deliberately NOT reset on login/logout — they belong to the physical
device/install, not to who happens to be logged in.

**New in `platform-kernel`**: `ObjectId`/`OrganizationId`/etc. had
`new_random()` and `Display` but no way to parse a UUID string back
into the type — genuinely missing, not overlooked elsewhere (grepped
first). Added `from_uuid_str` plus a real `FromStr` impl (so
`str.parse::<ObjectId>()` works idiomatically too), implemented once
and shared by both, so there is exactly one parsing rule. This is what
turns `/api/auth/login`'s JSON `organization_id` string back into a
real id client-side.

**Verified for real, not assumed**:
- `cargo check -p platform-kernel` clean; `cargo test -p platform-kernel`
  31/31 passing (28 pre-existing + 3 new: round-trip, `FromStr`
  parity, garbage-input rejection).
- `cargo check -p desktop-shell` clean — confirmed via a real, timed
  compile in this sandbox (~6 minutes, genuine Tauri/GTK dependency
  tree, not skipped).
- New unit tests written for `session.rs` (save/load/clear round-trip
  via an in-memory `SecureStorage` fake — the real `KeyringSecureStorage`
  needs an actual OS credential store and can't run in this sandbox;
  corrupt-stored-data handling) and `lib.rs`
  (`relay_ws_endpoint_from_http` http→ws, https→wss, already-ws
  passthrough).

**Explicit, NOT verified — this is the important gap**: those new unit
tests (`cargo test -p desktop-shell --lib`) were written but never
confirmed passing. This sandbox's disk repeatedly ran out of space
mid-build while linking the full Tauri/GTK dependency tree (a `target/`
directory this size — several GB — is inherently disk-heavy, and this
environment's available space did not reliably survive a full test
build even after being cleared multiple times). `cargo check` (type
and borrow-checker correctness) passed cleanly and repeatedly; `cargo
test` (does the new logic actually behave correctly at runtime) did
not get a confirmed clean run. Anyone continuing this work should run
`cargo test -p desktop-shell --lib` on a machine with normal disk
headroom before trusting these tests pass — do not assume they do
because the code compiles.

**Not done, explicit gap**: no frontend login screen exists yet for
`desktop-shell` (React side) — the Rust commands (`login`/`logout`/
`get_current_session`) are ready to be called, but nothing in
`crates/bins/desktop-shell/ui/src` calls them yet, and `App.tsx`'s
route table still goes straight to `Dashboard` with no auth gate.
`admin-shell`'s server-settings screen (`ServerConnectionSettings`,
`ServerAddressField`) has not yet been reconciled with this new
`desktop-shell` session model — they are two separate, currently
unrelated pieces of work. Neither Windows nor Linux automation scripts
for full server setup exist yet. CI (`release.yml`) still only builds
`desktop-shell`, not `admin-shell`, and has not been updated for any
of this session's changes.

---

## Desktop authentication UI, server setup automation, and Admin release builds — 2026-08-19

### Inherited native-session tests — now verified

The handoff correctly marked the new `desktop-shell` session tests as unverified: the earlier environment had run out of disk space while linking the full Tauri/GTK test target. This was not treated as a compile-only guarantee. The continuation environment was provisioned with the repository-pinned Rust 1.97.1 toolchain, a C/C++ linker, GTK/WebKit dependencies, and a live Linux Secret Service session; then it ran `cargo test -p platform-kernel` followed by `cargo test -p desktop-shell --lib`.

`platform-kernel` completed successfully, allowing the chained desktop test to begin. The final desktop run completed with **11 passing tests and 1 explicitly ignored test**. The new session persistence tests (`save_then_load_round_trips_every_field`, clear/no-session behavior, and corrupt-data error handling) all passed, as did the relay-endpoint conversion tests. The ignored test remains the existing live keyring write round-trip: it requires an interactive keyring-unlock prompt that a headless test environment cannot satisfy. The two non-mutating keyring tests were run against a **real system-installed `gnome-keyring-daemon` Secret Service provider** in a newly created `dbus-run-session`; no mock or stub was used. The session did **not** have a pre-unlocked interactive login keyring, so this evidence establishes the adapter's `None`/`NotFound` handling for an unknown key against a reachable real OS service—not successful writable-keyring access. The latter remains explicitly unverified in headless CI, and the write round-trip remains ignored for that reason. The prior initial failure was therefore diagnosed as a missing credential-store provider in the bare environment, not papered over by weakening the tests.

### `desktop-shell` React login, authenticated session, and connection settings — complete

**The frontend no longer creates placeholder identity.** `App.tsx` now calls the real native `get_current_session` command before rendering any operational page. A missing session is normal and renders the new Login screen; an IPC/storage failure is rendered as an explicit retryable startup error, not silently treated as an unauthenticated state. Once authenticated, every page is rendered below a `SessionProvider`; `useSession()` has no random-ID fallback and throws if it is used outside that authenticated tree. This removes the old frontend/backend organization mismatch rather than merely hiding it.

**The native session IPC shape was extended deliberately, without exposing tokens.** Existing desktop command envelopes require `organizationId`, `userId`, and `deviceId`; the prior `SessionInfo` exposed only the organization. The native result now includes the real non-secret user id and the managed local replica identity as UUID text, in addition to username, admin flag, server address, and organization. Access and refresh tokens remain exclusively in `StoredSession` and `SecureStorage`. The provider converts UUID text back into the transparent 16-byte array shape used by the existing Tauri command IPC, so the existing Dashboard, Missions, Tasks, Approvals, Messaging, Files, and Notifications command paths consume the real identity without a separate frontend-only convention. A new Rust unit test proves the serialized result contains the real actor/device fields and omits both token fields.

**Server configuration follows the session model rather than copying Admin’s localStorage model.** The Login page uses the same collapsed Server address / connection settings visual pattern as `admin-shell`, with username, password, and server address available before a successful login. A health check can validate the address, but an address is persisted only by a successful native login, because it belongs with the server-specific token bundle. The authenticated Settings page displays and tests the current address; choosing a changed address first proves it reachable, then clears the current session and returns to Login for a new authentication. Reusing tokens from one server against another would be a correctness and security error, so a silent hot-swap was deliberately rejected. Logout is available in the persistent application chrome and continues to leave the local SQLite data and managed replica state untouched, as decided in the preceding native-login entry.

**Verification:** `cargo fmt --all` followed by the desktop library tests above; `npx tsc -b`, `npx vite build`, and `npx oxlint` in `desktop-shell/ui`, all clean. There is no desktop frontend test harness in this workspace, so no fictitious browser test suite was added. A live login screen run also requires an actual configured API server and native WebView session, which was not available inside the build sandbox.

### Windows and Debian full server setup scripts — complete

Two new scripts install a **provided, trusted prebuilt** `api-server` binary rather than downloading or compiling an executable. Both protect existing SQLite data by default, create the first Admin only on a fresh database through the real one-time bootstrap endpoint, remove the bootstrap token before their final state, poll `/health`, and issue a real `/api/auth/login` request with the created or supplied Admin credentials. This is intentionally stronger than merely starting a process and assuming it works.

On **Windows**, `scripts/setup-onyx-windows.ps1` requires elevation, uses only a **Process-scoped** PowerShell execution-policy bypass, installs under `C:\ProgramData\ONYX`, writes a named and verified `New-NetFirewallRule` for the chosen API port and binary, and defaults that rule to Domain/Private profiles plus `LocalSubnet`. Public-network exposure requires an explicit `-AllowPublicNetwork`. The service choice is opt-in: `-InstallService` requires a separately obtained, trusted NSSM executable and never downloads one. When selected, it configures the API to run as `NT AUTHORITY\LocalService`, grants write access only to ONYX’s data/log locations, starts automatically, and is verified as running. A regular background process is the default and the script states plainly that it will not automatically survive reboot or recover from failure. This script was statically reviewed but cannot be executed from the Linux sandbox; Windows Firewall, NSSM, and the Windows service manager remain real-machine verification requirements.

On **Debian 13**, `scripts/setup-onyx-debian.sh` installs its minimal setup/verification prerequisites (`ca-certificates`, `curl`, `jq`, and `openssl`), creates an unprivileged `onyx` account, and installs the binary in `/opt/onyx` with data and logs in `/var/lib/onyx` and `/var/log/onyx`. The runtime dependency conclusion was checked against `deploy/docker/api-server.Dockerfile`: ONYX itself uses `rustls` and does not require a runtime OpenSSL package; `curl`, `jq`, and `openssl` are script utilities for health/login checking and safe token generation. UFW is changed only if it is installed and active; its default rules allow the selected TCP port only from private IPv4 ranges, while `--allow-public-network` is a deliberate broadening. `--install-service` creates a hardened systemd unit that runs as `onyx`, restarts on failure, starts at boot, reads a root-owned environment file, and has only the database/log paths writable. The default remains a documented regular background process, not an implied production supervisor.

**Verification:** `bash -n` passed, then the Debian script was run end-to-end against a freshly built real `api-server`, an isolated SQLite database, and an isolated loopback port. It installed the binary, initialized/migrated the database, passed health, created the first Admin, restarted without the bootstrap token, and passed a real login verification. The test server and temporary data were then removed. The separate runbook `docs/runbooks/automated-api-server-setup.md` documents invocation, service implications, firewall scope, and recovery behavior.

### Release workflow now builds both desktop shells — complete locally, pending real GitHub runner evidence

`release.yml` now contains a separate `release-admin-desktop` job that mirrors the already-reviewed `release-desktop` matrix for Linux, macOS, and Windows. A separate job was chosen instead of a parameterized two-app matrix because the apps have distinct frontend lockfile cache paths, application metadata (`ONYX` / `com.onyx.platform` versus `ONYX Admin` / `com.onyx.admin`), and likely future signing/release requirements; the explicit job keeps those differences auditable without making path logic conditional and opaque.

The job uses the same established workspace-root bundle location, Linux GTK/WebKit dependency set, `tauri-apps/tauri-action` build-only mode, and bundle-file search policy as the Staff job. It builds `deb`/`AppImage` on Linux, **DMG only** on macOS (not the `.app` directory), and MSI/NSIS on Windows; it uploads distinct `admin-desktop-*` artifacts. The final `publish` job now waits for the Admin job as well. The existing signing caveat deliberately remains true for both applications: no real Apple Developer/notarization or Windows code-signing credentials are present, so macOS ad-hoc fallback remains non-blocking and user-facing Gatekeeper/SmartScreen warnings remain an explicit release gap.

**Verification:** the workflow parses as YAML and a local structural check confirms the three-platform Admin matrix, Tauri build action, and `publish` dependency. `admin-shell/ui` completed `npm ci`, `npx tsc -b`, `npx vite build`, and `npx oxlint` with zero warnings or errors. This still does not replace real cross-platform GitHub Actions execution; the next commit is intended to be pushed with a dedicated `v*` validation tag so the actual runners can surface any platform-specific bundling issue.


---

## Cross-platform Tauri CLI invocation in `release.yml` — verified 2026-08-20

**The original defect was a dependency-resolution failure, not a Rust or
Tauri compilation failure.** The Staff and Admin frontend dependencies are
installed independently under their respective `ui` directories, while the
release action runs from the repository workspace. The original
`tauriScript: npx tauri` therefore did not reliably discover either
UI-local `@tauri-apps/cli` package. A manual, non-release
`workflow_dispatch` run on the branch-renamed baseline failed all six
desktop matrix entries before Rust compilation or bundle creation with
`npm error could not determine executable to run`; the three server-binary
matrix entries were unaffected. This was observed on GitHub-hosted runners,
not inferred from the workflow text. [1]

**The first correction was locally valid but incompatible with the pinned
action version.** It replaced the bare `npx` command with
`npm --prefix <ui-directory> exec tauri`, which resolves the CLI correctly
when executed directly. Both UI installations passed `npm ci`, and both
exact direct command shapes produced Tauri 2.11.4 `build --help` output
locally. However, the workflow uses `tauri-apps/tauri-action@v0`, not the
current development implementation initially reviewed. The exact v0 runner
treats an `npm` executable specially and inserts `run` unless its first
argument is already `run`. As a result, the action transformed the supplied
command into `npm run --prefix <ui-directory> exec tauri build -- ...`, an
invalid npm invocation. The subsequent manual retry consistently failed the
completed Staff and Admin desktop jobs on Linux, macOS, and Windows for
that reason, while the server-binary jobs passed. [2] [4]

**Decision: invoke the installed CLI module through Node directly.** Both
desktop jobs now set `tauriScript` to the platform-neutral command below,
with the shell-specific UI directory substituted in the workflow:

```yaml
tauriScript: node ${{ github.workspace }}/crates/bins/<shell>/ui/node_modules/@tauri-apps/cli/tauri.js
```

This deliberately bypasses the action's npm-specific command rewriting and
also avoids relying on the platform-specific executable shim in
`node_modules/.bin` (`.cmd` on Windows versus a POSIX shim elsewhere). The
v0 runner receives `node` as its executable, passes the package's real
`tauri.js` entry point as the first argument, and appends the requested
`build`, target, and bundle arguments normally. The two workflow edits are
limited to the Staff and Admin `tauriScript` values plus explanatory
comments. [4] [5]

**Verification was repeated on real GitHub-hosted runners without a release
tag.** `actionlint` and `git diff --check` passed before the workflow commit
`c2eb740b9e902044909ad50befffcb13104085a8`; direct local invocations of
both installed CLI entry points also returned `build --help`. The full
manual debug-build matrix for that commit was run as GitHub Actions run
`32311213443`, so no `v*` tag was created, no GitHub Release was published,
and the image/publish jobs correctly remained skipped. [3]

| Target class | Linux | macOS | Windows | Result relevant to this decision |
|---|---|---|---|---|
| Server binaries | Passed | Passed | Passed | Unchanged control matrix; all three platform jobs succeeded. |
| Staff desktop | Passed | Reached Tauri compile and DMG bundling | Passed | The direct Node command invoked Tauri successfully on every platform. |
| Admin desktop | Passed | Reached Tauri compile and DMG bundling | Passed | The direct Node command invoked Tauri successfully on every platform. |

The Linux and Windows desktop artifacts completed successfully for **both**
applications. On macOS, both applications compiled successfully through the
direct Node CLI and reached Tauri's DMG bundling phase. They then failed at a
separate asset-processing step: `Failed to create app icon: Format error
decoding Ico: The PNG is not in RGBA format!` This establishes that the
cross-platform CLI invocation no longer blocks macOS; the remaining macOS
DMG failure is an icon-format defect, not a command-resolution or signing
failure. It was deliberately **flagged but not changed** in this scoped
workflow repair: correcting or replacing application artwork is a separate
product/asset change and was not silently folded into the CI fix. The
workflow run is consequently overall red only because of those two macOS
icon failures, not because `tauriScript` failed to resolve or execute. [3]

**Remaining explicit gap:** a future scoped change must supply an RGBA-valid
PNG/ICO input for both macOS application bundles and then rerun the macOS
matrix to obtain final DMGs. Apple notarization and Windows code-signing
credentials also remain separate, pre-existing release concerns; neither
was invented or altered here. The direct-Node workflow fix itself is now
verified with complete server coverage and successful Linux/Windows desktop
artifacts for both shells, plus macOS compilation/bundling reachability on
real hosted runners.

### References

[1]: https://github.com/So-Muzaff/Onyx-Framwork/actions/runs/32299530139 "Initial manual release debug-build run"
[2]: https://github.com/So-Muzaff/Onyx-Framwork/actions/runs/32301698085 "npm-prefix retry run"
[3]: https://github.com/So-Muzaff/Onyx-Framwork/actions/runs/32311213443 "Direct-Node verification run"
[4]: https://raw.githubusercontent.com/tauri-apps/tauri-action/v0/src/runner.ts "tauri-action v0 runner implementation"
[5]: https://github.com/So-Muzaff/Onyx-Framwork/blob/c2eb740b9e902044909ad50befffcb13104085a8/.github/workflows/release.yml "Verified direct-Node release workflow change"


---

## UI/UX remediation — Milestones 1 and 2 — Complete

This increment implements the approved first two remediation milestones from
the UI/UX audit: restore operator trust and accessible interaction first, then
align the Browser Remote Operator, Staff desktop shell, and Admin desktop shell
around stable state, error, navigation, and decision patterns. The work was
kept intentionally within the audited defects. It does **not** change domain
state machines, authorization decisions, release publication behavior, token
storage architecture, or any unrelated product workflow.

**Current documentation was consulted before focus-sensitive implementation.**
The configured Context7 React documentation source was queried for controlled
dialog and focus/effect lifecycle guidance. Its current `useEffect` examples
use an effect to synchronize controlled modal state and require cleanup for
imperative dialog effects. The Staff `AccessibleDialog` and browser navigation
implementation follow that pattern: they record current focus when opening,
move focus to the controlled surface, register keyboard listeners only while
open, remove listeners on cleanup, and restore focus to the initiating control
when the surface closes. This was used as implementation guidance, not treated
as a substitute for project-specific validation. [1]

**Browser trust and state fixes.** The dark login hero now explicitly applies
white foreground colour to its heading, preventing the global navy `h1` rule
from creating the previously measured 1.16:1 dark-on-dark contrast failure.
A shared `ProjectionState` primitive now gives browser operational pages a
single `loading` / `unavailable` / `stale` / `empty` / `ready` contract. The
Dashboard, Missions, Tasks, Notifications, and Approvals pages use it so a
failed first load is rendered as an unavailable projection with recovery
rather than a false `0 total`, empty portfolio, or actionless blank region.
Stale responses retain usable data with explicit freshness messaging.

**Organization context decision and implementation.** The current login API
returns an `organization_id` but not an authoritative organization display
name. The hard-coded `ONYX Test Operations` label was therefore removed. The
browser session model now accepts an additive optional
`organization_display_name` for forward compatibility; when absent, the UI
shows a neutral context derived from the authenticated organization ID prefix
and says “Verify organization before acting.” This is deliberately truthful:
no tenant name is inferred from a user record or fabricated from test copy.
A future additive backend organization-display field remains required to
replace the neutral fallback universally.

**Accessible navigation and decision surfaces.** The browser mobile sidebar
now has breakpoint-aware `inert` and focus behavior: closed drawer links are
not keyboard-reachable, the trigger reports its expanded state, opening moves
focus to the first navigation link, Escape closes, closing returns focus to
the trigger, and selecting a route moves focus to main content. The Staff
shell gained equivalent narrow-window navigation rather than compressing all
work behind its permanent rail. Staff approvals now use a reusable named
modal primitive with modal semantics, description linkage, initial focus,
Tab/Shift+Tab containment, non-busy Escape behavior, and focus restoration.
The browser approval dialog gained the same focus lifecycle protections.

**Approval evidence policy resolved.** The earlier clients disagreed: Browser
required an explanation only for rejection while Staff required one for both
decisions. This implementation adopts the remediation plan's recommended
policy: **rejection requires a reason; approval accepts an optional decision
note**. The matching Browser and Staff controls now use that rule, labels, and
disabled-submit behavior. This is a client interaction-policy alignment; the
current server/domain validation contract was intentionally not altered in
this UI-only increment.

**Raw user-facing errors replaced at shared boundaries.** Browser and Admin
HTTP helpers now map network, session, permission, conflict, validation,
not-found, service, and unexpected failures to stable user-safe copy and a
recovery action. The Staff shell gained the same `UserFacingError` shape for
startup, shared query, shared command, login fallback, and logout errors.
Existing pages that render shared query/command `error.message` now consume
that mapped copy without per-page raw exception handling. This resolves the
live Staff startup failure that previously rendered a raw `TypeError` inside
a scrolling `<pre>`. Specialized local input/parsing messages are not silently
suppressed; they remain a follow-up review item, but no longer define the
central server/IPC failure surfaces audited as high risk.

**Admin first-run connection flow refined.** The pre-login connection setting
was extracted into an explicit component. Operators choose either “This
computer” (loopback) or “Another computer” (LAN address), test the address,
and save it only after a successful five-second `/health` request. The entered
address is retained but not saved on failure, and the resulting recovery state
is announced in-page. This preserves the existing correct persistence
semantics while making first-run intent and result feedback more scannable.

**Verification:**
```
web-ui: npm run type-check                         # clean
web-ui: npx vitest run tests/unit/projection-state.test.tsx
                                                    # 5 tests, 0 failures
web-ui: npm test                                   # 138 passed, 7 skipped;
                                                    # 1 pre-existing real-server suite skipped
web-ui: npm run build                              # clean; existing bundle check passed
crates/bins/desktop-shell/ui: npm run build        # clean (tsc -b + Vite)
crates/bins/admin-shell/ui: npm run build          # clean (tsc -b + Vite)
git diff --check                                   # clean
```
The full browser suite continues to emit the pre-existing jsdom/axe canvas
warning while still passing its accessibility assertions. That limitation is
explicitly retained for Milestone 3's real-browser accessibility/visual test
work; it is not represented as evidence that browser-rendered contrast has
been automatically proven. Live local browser inspection independently
confirmed the white login hero, neutral organization-ID context, and an
unavailable Mission projection without false-zero copy. Only synthetic local
session storage was used; no real user, organization data, or production
server was accessed.

A complete file-by-file change inventory, implementation rationale, Context7
provenance, validation record, and deferred dependencies is stored in
`docs/ui-remediation/MILESTONES_1_2_IMPLEMENTATION.md`.

**Explicit remaining scope:** Milestones 3 and 4 remain unimplemented:
tracked ESLint restoration, real-browser/Playwright and visual regression
coverage, native runtime smoke evidence, CI enforcement, and release evidence
retention. The authoritative backend organization display-name field is also
outside this increment. These are flagged rather than silently represented as
complete.

### References

[1]: https://react.dev/reference/react/useEffect "React useEffect reference (consulted through Context7)"


---

## UI/UX remediation — Milestones 3 and 4 — Complete

This increment makes the earlier UI/UX remediation enforceable rather than
merely documented. It restores the Web UI's declared lint command as a tracked,
deterministic zero-warning gate; adds real Chromium browser coverage for the
highest-risk audit repairs; records native frontend smoke evidence as a
machine-readable artifact; and integrates those checks into ordinary CI. It
does **not** change tag-triggered publication or silently absorb the separate
macOS icon asset defect.

**Lint became a real quality gate.** `web-ui/package.json` already declared
`npm run lint`, but the repository had no tracked ESLint configuration, so the
command was not reproducible after a fresh checkout. This increment adds
`web-ui/.eslintrc.cjs` with TypeScript and React Hooks recommended rules,
explicit generated-artifact exclusions, and `--max-warnings=0`. The first
real run found two genuine dead declarations: draft item state in
`TodoTargets/ListDetail.tsx` and an unused generic parameter in the
projection-state test helper; both were removed without changing behavior.
The installed TypeScript compiler was 5.9.3 while the old
`@typescript-eslint` v6 parser printed an unsupported-version warning. The
parser and plugin were upgraded to v8 in the committed manifest/lockfile, and
the lint gate now passes without that warning.

**The browser gate covers UI behavior jsdom cannot prove.** Playwright and
`@axe-core/playwright` are now locked development dependencies. Its two
projects exercise desktop Chromium and an iPhone-sized Chromium viewport. The
explicit Chromium override matters: an iPhone device descriptor defaults to
WebKit, which the first local test run exposed; the configured gate is
intentionally Chromium-only to match its installed CI runtime. The test
server uses an isolated port (4179) and refuses to reuse a pre-existing dev
server, preventing a test from accidentally checking a different checkout.

The test fixture uses only a synthetic session and only intercepts the exact
`/api/query` pathname. This exact-path requirement was itself caught by
real-browser diagnosis: the broad pattern initially chosen also matched Vite's
`src/api/query.ts` module and served JSON as a JavaScript module, producing a
blank protected route. The corrected fixture preserves Vite module loading and
makes test data strictly local. The committed browser assertions now cover the
white browser-login hero heading plus axe scan, mobile drawer focus movement
and Escape/trigger restoration, failed Mission projection recovery rather
than a false empty portfolio, and named/keyboard-accessible approval dialog
behavior with the aligned decision-note policy.

**Visual regression evidence is intentionally cross-environment stable.** The
initial full-hero screenshot passed locally but failed on GitHub-hosted Linux
because font metrics wrapped the same heading differently (10% of pixels) even
though the live CSS foreground assertion and axe check both passed. Instead of
loosening image-diff tolerance until a real change could slip through, the
visual baseline now snapshots only the background/layout after semantic text
colour and real-browser axe checks run on the actual visible text. This is a
deliberate separation: text accessibility stays semantically enforced, while
the pixel baseline catches deterministic background/layout changes without
being a font-rendering proxy. The first run and correction are both recorded
in the Milestone implementation record, not hidden.

**Native UI smoke evidence has an explicit runtime boundary.** The new
`scripts/collect-ui-evidence.mjs` records schema version, commit, runner,
commands, result, and GitHub IDs; critically, its `runtime_claim` states that
a frontend smoke build does not assert native window launch. The new
`native-ui-evidence` job runs locked dependency installation and production
builds for both Staff and Admin frontends after Web quality succeeds, writes
that manifest, and retains it as a 30-day CI artifact. The complementary
`docs/ui-remediation/NATIVE_UI_SMOKE_EVIDENCE.md` defines frontend,
Tauri-matrix, and human target-native evidence levels so artifact consumers do
not overstate what was observed. It also preserves the no-tag release boundary
and records the known macOS RGBA icon defect as a separate asset limitation.

**CI integration is limited and deliberate.** The existing Web job now runs
locked install, lint, type check, existing Vitest/axe/feature suites, Chromium
installation, browser regression, production build/bundle budget, and a
14-day report artifact. A separate native evidence job is dependent on that
Web quality result. `release.yml`, its manual-debug behavior, and all `v*`
tag publication semantics remain untouched: ordinary CI has no release side
effect.

**Verification:**
```
web-ui: npm run lint                    # clean, zero warnings
web-ui: npm run type-check              # clean
web-ui: npm test                        # 138 passed, 7 skipped; 1 existing suite skipped
web-ui: npm run test:browser            # 4 passed, 4 project-inapplicable skips
web-ui: npm run build                   # clean; existing bundle check passed
Staff UI: npm ci && npm run build       # clean
Admin UI: npm ci && npm run build       # clean
node scripts/collect-ui-evidence.mjs …  # schema 1.0 manifest verified
actionlint .github/workflows/ci.yml     # clean
git diff --check                        # clean
```

**Hosted validation was read back from GitHub, not inferred.** Initial commit
`4e062c1` triggered CI run `32332728269`; the new Web job completed every
quality stage except the font-sensitive full-hero visual baseline and uploaded
its report artifact (`9393590353`). The documented stable-baseline correction
`bee8798` then triggered run `32333049774`: the Web job passed and uploaded
artifact `9393694749`; the dependent native UI evidence job passed and uploaded
artifact `9393702754`. Downloading that artifact confirmed `status: passed`,
commit `bee8798cfc73718788e3d239935588f39dc81edb`, Linux x64, GitHub run ID
`32333049774`, and the two exact Staff/Admin smoke commands. [1] [2]

**Explicit unrelated limitation:** the overall hosted CI run remains red
because the pre-existing Rust `check` job fails `cargo fmt --check` on Rust
files not modified by these two commits, including
`crates/bins/api-server/tests/team_leader_precheck_authorization.rs`. This is a
rustfmt-only diff; Web quality and native evidence both passed. It is flagged,
not silently fixed, because repairing unrelated Rust formatting exceeds the
approved UI quality scope. Package-manager audit output also surfaced existing
transitive advisories (Web: 7; Staff: 1; Admin: 0); automatic audit fixes are
similarly deferred to a compatibility-tested security dependency update.

The complete artifact contract, local validation record, hosted evidence
links, and file-level implementation detail are stored in
`docs/ui-remediation/MILESTONES_3_4_IMPLEMENTATION.md`.

### References

[1]: https://github.com/SMozaff/Onyx-Framwork/actions/runs/32332728269 "Initial Milestone 3/4 hosted validation"
[2]: https://github.com/SMozaff/Onyx-Framwork/actions/runs/32333049774 "Stable visual baseline and native evidence validation"


---

## Native desktop visual redesign — Staff and Admin shells — Complete

The Staff desktop shell (`crates/bins/desktop-shell/ui`) and the distinct Admin
desktop shell (`crates/bins/admin-shell/ui`) have been redesigned from the
current minimal dark-console presentation into a shared ONYX operational
workspace. The user supplied two ONYX reference compositions: a deep-navy,
left-navigation operations view with a white/blue-grey workspace, and a
split, navy-and-white sign-in view. The implementation adopts that visual
language rather than copying browser-only product claims into native clients.

**Shared design system:** Both native `index.css` files now define the same
light operational tokens: blue-white work canvas, white cards, navy primary
text, blue primary actions, readable semantic status colours, visible
focus-ring treatment, a deep-navy branded side plane, high-contrast active
navigation state, connection/session chips, and split authentication layout
primitives. Existing semantic Tailwind class names (`onyx-text`,
`onyx-surface`, `onyx-status-blocked`, and so on) were preserved and remapped
rather than globally replacing page markup. This deliberately restyles the
many existing card, form, table, dialog, safe-error, and status surfaces while
keeping their behavior and their semantic state mapping intact.

**Staff desktop:** `MainLayout` now presents the authenticated native workspace
as an ONYX staff-operations shell: branded sidebar, contextual organization
header sourced from the authenticated session, session identity, visible
connection chip, and an offline-only alert banner. The alert says that commands
are not queued only when the actual Tauri sync-status value reports offline;
it does not manufacture a connectivity state. All prior responsive drawer
behavior is retained: the button reports expanded state, the closed nav becomes
inert, first-link focus is restored on open, Escape closes the drawer, and
focus returns to the trigger. Existing sync polling, conflict/pending counts,
logout handling, and safe mapped logout error remain unchanged.

The Staff login now uses the split visual composition while retaining every
pre-existing authentication and recovery behavior: `login` still uses the
native command, server-address validation still requires an HTTP(S) URL,
network failures expose the connection controls, the health test still avoids
persisting an unreachable address, and user-facing errors remain safely mapped.
The authentication form now has stronger labelled controls, an explicit native
session-security explanation, a visible primary action, and an accessible
expanded connection-settings state.

**Admin desktop:** The Admin shell receives the same workspace framing, but
truthfully remains a thin HTTP client: its header presents the authenticated
user and organization identifier held in its session, not a fictitious sync or
connection indication. The existing HTTP logout behavior and nested router
`Outlet` remain intact. Its login uses the matching split composition while
continuing to delegate server setup to `ConnectionSettings`, whose test-before-
save behavior is unchanged. The Users page, as the primary dense administrative
surface, was additionally refined with an identity-management heading,
labelled username/password/class controls, a clear administrator-access
checkbox, a stronger primary action, and a bounded white data table. No API
request shape, user-management behavior, or safe error path changed.

**Explicit design choices:**
- The native labels are deliberately `Staff operations` and `Administration`,
  not the reference browser client's `Remote Operator`, because the apps have
  different client contracts.
- Organization labels use authenticated identifier values shortened only for
  display, with the complete UUID in the title attribute; no hard-coded
  organization name was introduced.
- The Admin header's `Admin session` chip signals an authenticated UI session,
  not reachability. Server reachability remains the responsibility of the
  existing explicit test-before-save interaction.
- The user references informed colour, hierarchy, shell geometry, and
  authentication composition only. No reference image asset was copied,
  altered, or embedded in the application.

**Verification:**
```
Staff UI: npm ci && npm run build          # clean (TypeScript + Vite)
Admin UI: npm ci && npm run build          # clean (TypeScript + Vite)
Staff UI: npm run lint                     # 0 warnings, 0 errors
Admin UI: npm run lint                     # 0 warnings, 0 errors
git diff --check                           # clean
```

**Live visual evidence:** The Admin `/login` and authenticated `/users` shell
were rendered in a local Vite preview with synthetic browser-only session data;
the preview confirmed the navy branded access plane, white form/work surface,
active navigation contrast, organization/identity header, clear labelled
user-creation card, and safe API-failure message when no backend was present.
The Staff browser preview intentionally stopped at its existing safe native
startup-error view: unlike Admin, Staff correctly invokes Tauri's
`get_current_session` before it exposes login or operational content, and a
plain browser has no native command provider. This is a verification boundary,
not a fallback to fake desktop session data. The production Staff build passed;
Tauri-window visual launch remains a separate native-runtime evidence level.
The full preview record is in
`docs/ui-remediation/DESKTOP_REDIRECT_PREVIEW_NOTES.md`.

**Scope boundary:** This work intentionally does not change desktop Rust,
Tauri packaging, release workflows, server contracts, connection semantics,
or the separate production deployment blockers. The Staff UI's package audit
continues to report one existing high-severity dependency advisory after locked
installation; dependency remediation remains a separate compatibility-tested
security task.

## Fixed seeded admin account replaces token-gated bootstrap — Complete, security tradeoff explicit

**Context.** The only way to create the first admin account was previously
`POST /api/admin/bootstrap` (routes/admin.rs): disabled unless
`ONYX_BOOTSTRAP_TOKEN` is set on the server, constant-time token check,
refuses forever once any user exists. This blocked a Windows Codex agent
session trying to establish admin-shell credentials — its own tool hit a
Windows Credential Manager "secret longer than platform limit" error
(2,560-char native secret-storage cap) while attempting to store/generate a
token for this flow. Rather than debug that tool-side error, the person
explicitly instructed: define a fixed username/password for the admin
platform and remove the token-bootstrap requirement, and to not worry about
the security implications of doing so.

**What changed.** `ApiState::new` (routes/mod.rs) now seeds a fixed admin
account automatically on first startup, exactly once, guarded by
`user_store.count().await? == 0` — the same one-time semantics the token
bootstrap used, just automatic instead of requiring a token + HTTP call.

- Username: `All-Father` (as given)
- Password: `passvord0000` — **not** the literal `passvord` given. The
  shared `PasswordHasher::hash()` (security-adapter/src/password.rs) enforces
  a hard 12-character `MIN_PASSWORD_LENGTH` before hashing anything, and
  `passvord` is 8 characters. Calling it as given would return `Err`, and
  because this runs during server startup before `ApiState::new` returns,
  that error would abort the entire api-server boot, not just fail the seed
  step. There is no lower-level bypass of that policy exposed from
  `routes/mod.rs` without editing `PasswordHasher` itself — a shared
  primitive used by every other account's password, judged a broader and
  riskier change than a single seeded login. Resolution: the literal
  8 characters requested, with a fixed, visible `000` appended solely to
  clear the 12-character floor. Disclosed here rather than silently
  substituted.
- Stored via the same Argon2id `PasswordHasher` path every other account
  uses — only the plaintext choice is fixed, not the hashing/storage
  mechanism.
- `is_admin: true`, `is_manager: false`, `class: None`, `parent_user_id: None`.

**Security tradeoff — stated plainly, not glossed over.** This removes the
fail-closed, token-gated, one-time protection the bootstrap endpoint
provided for the *first* admin account. That account now exists with a
fixed, publicly-known (to anyone with this source or binary) password the
moment the server starts against an empty database — no token, no HTTP call,
no secret required. This is acceptable *only* for the internal, non-public
office test-drive this milestone targets. It was implemented as explicitly
instructed despite this tradeoff being raised beforehand.

**What was NOT touched.** `/api/admin/bootstrap` itself (routes/admin.rs) is
untouched and still works exactly as it did before, for any *additional*
account beyond the seeded one. It will correctly refuse to create a second
"first" admin once the seeded account exists (`BOOTSTRAP_ALREADY_COMPLETED`),
same as it always has once any user exists — this is pre-existing behavior,
not new. No frontend (`admin-shell` or `desktop-shell`) code was touched;
this is a backend (`api-server`) seeding change only. `seed_if_empty`'s
existing mission/task/notification/approval/report fixture seeding is
unchanged and unrelated — this is a separate seed added after `user_store`
and `password_hasher` both exist (`seed_if_empty` only has a raw SQLite pool
and runs earlier, before either exists, so the new logic could not be added
there).

**Not yet verified — real build/run still needed.** This has been written
and read back for compile-plausibility (types, trait signatures, and
`?`-conversion paths for `UserStoreError`/`PasswordError` were checked
against source, not assumed) but has **not** been compiled, run, or logged
into on a real machine. Per standing project rule, verification happens via
Manus AI's sandbox or the `Debug.yml` GitHub Actions workflow, not in this
environment.

## Two pending fixes: `mobile-core` workspace build break, and admin-shell's generic error messages — verified 2026-08-23

Two bugs were reported as already diagnosed but not yet landed. Checked
`git log`/`git show` against `origin/main` first, per standing project
rule — neither fix was present. Both are applied here, and both were
actually compiled/built in this environment, not just read for
plausibility.

**Bug 1 — `mobile-core` broke `cargo check --workspace`.** When
`client-composition::AppState::new` became `async` and
`AppStateConfig` gained a required `blob_store_root: std::path::PathBuf`
field (Phase 1, file-upload support for desktop-shell — see that
change's own doc comment on `AppState::new`), the doc comment claimed
every *existing* call site was updated. `crates/mobile-core/src/lib.rs`'s
`mobile_core_new` was not yet a call site at the time and was missed. As
a Cargo workspace member, `mobile-core`'s compile errors
(E0063/E0609/E0308 — missing field, `.sync_agent` accessed on an
un-awaited future, and `Arc<impl Future<...>>` where `Arc<AppState>` was
expected) abort `cargo check --workspace` entirely, not just a
mobile-specific check — this is why it showed up in the shared `check`
CI job rather than an isolated mobile job.

Fix, inside `mobile_core_new`'s existing `runtime.block_on(async { ... })`
block (already an async context — confirmed, not assumed):
- `AppState::new(pool, app_config)` is now `.await`ed.
- `AppStateConfig` now gets a `blob_store_root` field, derived from
  `db_path` (`mobile_core_new`'s first FFI parameter — the Dart/Flutter
  side's chosen SQLite file location) as `db_path`'s parent directory
  joined with `"blobs"`. This intentionally mirrors
  `crates/bins/desktop-shell/src/lib.rs`'s own
  `data_dir.join("blobs")` line exactly, rather than inventing a new
  convention for the same concept. Falls back to the relative path
  `"onyx-blobs"` only if `db_path` has no parent component (e.g. a bare
  filename with no directory), which should not happen given how the
  Flutter side is expected to supply this path, but is handled rather
  than left to panic.

**Bug 2 — admin-shell showed a generic "failed to X" message instead of
the server's real, specific error.** `api-server`'s `ApiError::into_response`
(`crates/bins/api-server/src/routes/mod.rs`) returns
`{ error: { code, category, safe_details: { message }, correlation_id } }`.
`crates/bins/admin-shell/ui/src/utils/errorHandler.ts`'s `normalizeError`
already read this shape correctly, and `useCommand.ts`/`useQuery.ts`
already used it. But `Users.tsx` (5 call sites: refresh, create, set
class, set parent, activate/deactivate), `Profiles.tsx` (3: refresh,
save, batch import), and `Settings.tsx` (2: create policy, apply legal
hold) either had a bare `catch {}` hardcoding a generic string, or read
`e.response.data.message` — a path that does not exist anywhere in the
real response shape, so it was always `undefined` and fell through to
the same generic fallback regardless of which specific error
(`USERNAME_TAKEN`, `INVALID_CLASS`, `WEAK_PASSWORD`,
`PARENT_USER_NOT_FOUND`, `PARENT_CYCLE`, `CANNOT_DEACTIVATE_SELF`,
`CLASS_REQUIRED`, etc.) the backend actually sent.

Added `describeError(error: unknown): string` to `errorHandler.ts` —
calls the existing `normalizeError`, then formats
`` `${commandError.code}: ${message}` `` when a `commandError` is
present (else just the message), matching the diagnosable format asked
for (e.g. `"USERNAME_TAKEN: That username already exists"`). All 10
call sites listed above now use it. `types/command.ts`'s
`CommandError['category']` union was missing `'VALIDATION'`, which
`api-server`'s admin routes (`routes/admin.rs`) send constantly for
exactly these errors — added.

**Deliberately not touched.** `Settings.tsx`'s and the connection-setup
component's raw-`fetch`-based `testConnection` helpers, which collapse
any failure to a boolean `/health` reachability check — not part of this
bug, and changing them to surface a server message would be wrong (a
`/health` probe failing has no `CommandError` to report). `Login.tsx`'s
login-submit catch was also left alone: it deliberately shows a generic
"Invalid username or password" on non-network failures rather than the
server's specific reason, which is correct security practice for an
auth endpoint (don't reveal whether the username or the password was
wrong) — not an instance of this bug, so not "fixed" into leaking that
distinction.

**Verification actually run, not assumed:**
- `cargo check -p mobile-core` — clean.
- `cargo check --workspace --exclude desktop-shell --exclude admin-shell`
  — clean. The full unrestricted `cargo check --workspace` could not be
  run in this environment: `desktop-shell` and `admin-shell` (Tauri)
  pull in `gdk-sys`, which needs the system GTK3 `gdk-3.0` pkg-config
  file; this sandbox has no GTK dev libraries installed and package
  installation via `apt-get` failed here (mirror 404s), which is an
  environment limitation unrelated to this change — not a claim that
  the excluded crates were checked. Every crate that was previously
  breaking (`mobile-core` and everything it's a dependency of) is
  included in the exclusion-scoped check and is clean.
- `cargo build -p mobile-core` was not run separately — `cargo check`
  already exercises the same trait/type resolution that was failing;
  a full `build` was not additionally run due to time, and this is
  disclosed rather than implied.
- Android/iOS mobile CI jobs specifically were not run — no mobile
  toolchain in this environment. Flagged, not silently skipped.
- `crates/bins/admin-shell/ui`: `npx tsc -b` and `npx vite build` both
  ran clean after `npm install`.
- No live `api-server` instance was available in this environment to
  actually trigger a real `USERNAME_TAKEN` (or similar) rejection and
  confirm the rendered UI text end-to-end. The fix was verified by
  reading `normalizeError`/`describeError` against the exact response
  shape `ApiError::into_response` sends (confirmed against
  `routes/mod.rs` source), and by a clean typecheck/build — not by an
  actual browser-observed failure. Stated explicitly rather than
  implied as tested.

## Task/Mission approval authorization gap closed — direct-manager-only, verified 2026-08-23

**The gap.** `TaskDecisionHandler`/`MissionDecisionHandler`'s underlying
`decide()` calls checked only the aggregate's own status (must be
`Submitted`/`AwaitingApproval`/etc.) before applying `ApproveTask`,
`RejectTask`, `RejectApproval`, or `ActivateMission` — never *who* was
issuing the command. Confirmed directly in
`VerifiedAuthority::is_authorized` (`platform-kernel/src/authority.rs`):
a documented Increment-1 stub that unconditionally returns `true`,
deferred to "Increment 7" per a frozen-spec ruling. Practically, this
meant any authenticated organization member — with no real relationship
to a given task or mission — could approve or reject it. Todo/Target-
list verification and Staff Loans already have real, working authority
resolution (`api_server::verifier_resolution`); Task/Mission approval
never had an equivalent, and that module's own doc comment ("as built
2026-08-16") never claimed to cover it.

**Scope, confirmed decision-by-decision with the project owner, not
assumed.** Direct-manager-only authority — an Admin, or the specific
user who is the task/mission owner's direct manager per the org tree —
deliberately *not* the same loan/escalation widening
`verifier_resolution` also does for lists. If Task/Mission approval
later needs that widening, it should be asked for explicitly rather than
added speculatively.

**What's gated and what isn't, and why:**
- Task: `ApproveTask`/`RejectTask` are gated. `SubmitCompletion` is
  deliberately *not* gated — it's the task owner acting on their own
  work, not a decision about someone else's.
- Mission has no `ApproveMission` command at all (a naming assumption
  that would have been wrong to build against). The real commands are
  `RequestApproval` (`Planning → AwaitingApproval`, the owner's own
  action — not gated, same reasoning as `SubmitCompletion`) and
  `RejectApproval` (`AwaitingApproval → Planning`, a decision about
  someone else's work — gated). `ActivateMission` was found, while
  reading `mission_domain::aggregate` to build this, to bypass the
  whole workflow: it accepts `Planning`, `AwaitingApproval`, and
  `Review` as valid source states identically, with its own code
  comment stating the "Approval received" guard is a stub that always
  passes — meaning `ActivateMission` could activate a mission straight
  from `Planning`, skipping `RequestApproval`/`RejectApproval`
  entirely. Confirmed explicitly with the project owner that closing
  this bypass was in scope, not just the narrower `RejectApproval`
  case — `ActivateMission` is now also gated.

**Design.** A new optional `HasOwner` trait in `platform-contracts`
(`traits.rs`), implemented only by `Task`/`Mission` — not forced onto
every aggregate (`Notification`, `TodoList`, etc. have no single-owner
concept). A new `OwnerAuthority` trait plus an `owner_check` parameter
on `api_server::handle_command` (a caller-supplied `(extractor closure,
authority strategy)` pair, `None` for every one of the ~20 other call
sites that don't need it — every one of those was updated to pass
`None` explicitly, not left broken; factored into a `type OwnerCheck<A>`
alias after `cargo clippy -D warnings` flagged the inline tuple type as
`type_complexity`). `desktop-shell`'s new `HierarchyCache`
(`hierarchy.rs`) implements `OwnerAuthority` by fetching and locally
caching the org's reporting-line tree at login (`GET
/api/users/hierarchy`, new route in `routes/admin.rs`) rather than
requiring a live server round-trip at the moment of every approval —
`desktop-shell` has no local `UserStore` (its embedded `AppState` never
composed one) and this app's offline-first design, confirmed directly
with the person rather than assumed, is why a cached-at-login approach
was chosen over a live check. A new `DenyAllOwnerAuthority` fallback in
`client-composition` (`app_state.rs`) is used whenever no real strategy
is configured (`AppStateConfig::owner_authority: None` — the pre-login
placeholder `AppState`, and `mobile-core`, which has no approval UI at
all yet) — deliberately fails closed, not open: "no checker configured"
must never silently mean "allow everything."

**Delivery and integration, not a from-scratch build in this session.**
This fix's 14 changed files (13 modified + `hierarchy.rs` new) arrived
as a complete file-content snapshot from a prior session that had zero
compilation performed against a real toolchain (disk-constrained
sandbox, per that session's own explicit instruction). This session's
job was to apply, compile for real, and fix whatever a real compiler
found — treated as expected, not a sign the design was wrong, matching
this project's established pattern (e.g. the `mobile-core`
`blob_store_root`/`.await` fix earlier this same session). Real gaps a
compiler caught that the source-reading-only prior session could not:
- The delivered files had CRLF line endings throughout (a Windows
  editor round-trip) — normalized to LF before any real diff was
  possible; the true diff was ~450 lines across 13 files, not the
  ~13,000-line diff CRLF-vs-LF made `git diff --stat` initially show.
- `map_command_error` (`routes/command.rs`) had no explicit match arm
  for the new `CommandError::OwnerAuthorityDenied` variant — it fell
  through to the generic `other =>` catch-all, which would have
  returned HTTP 500/`INFRASTRUCTURE`/`TRANSIENT` for what is actually a
  403/`AUTHORITY`/`NON_RETRYABLE` rejection. Added an explicit arm
  (and to `command_error_class`'s metrics-label match, which was
  genuinely non-exhaustive and would not have compiled otherwise).
- Three `api_server::handle_command` call sites in
  `client-composition/src/file_upload.rs` (`UploadSession`/`FileAsset`
  commands) were missed by the scripted `None,` insertion the prior
  session used for the ~20 other call sites — a real `E0061` (wrong
  argument count), not a style issue. Added `None,` to each.
- `AppStateConfig` gained a required field but three test/production
  call sites elsewhere in the workspace were never touched by this
  delivery: `mobile-core/src/lib.rs`'s `mobile_core_new` (from this
  same session's earlier, unrelated fix — added `owner_authority: None`,
  same fail-closed reasoning as the placeholder `AppState`, disclosed
  in that function's own comment that mobile-core has no approval UI
  yet so this isn't a regression in practice), and
  `client-composition/tests/app_state_wiring.rs`'s `test_config()`
  (added `owner_authority: None`).
- `TaskDecisionHandler::new`/`MissionDecisionHandler::new` gained a
  required constructor parameter, but the existing
  `task_end_to_end_sqlite.rs`/`mission_end_to_end_sqlite.rs` integration
  tests (5 call sites total) were never updated — added a small
  `AllowAllOwnerAuthority` test double to `tests/support/mod.rs` (none
  of those tests exercise the owner-gated commands, so its behavior is
  inert for them; allow-all rather than deny-all specifically so a
  future test that *does* add coverage for those commands isn't
  silently blocked if it forgets to swap the double out).
- `cargo clippy --workspace -- -D warnings` (this repo's actual CI
  gate, confirmed by reading `.github/workflows/ci.yml`) flagged the
  inline `Option<(Box<dyn Fn...>, Arc<dyn OwnerAuthority>)>` tuple type
  as `type_complexity` in `handle_command`'s signature — factored into
  the `OwnerCheck<A>` type alias mentioned above and re-exported from
  `api_server`, used consistently at both call sites in
  `decision_handler.rs` too.

**`desktop-shell` could not be compiled or tested in this environment.**
It (and `admin-shell`) pull in `gdk-sys` via Tauri, which requires the
system GTK3 `gdk-3.0` pkg-config file; this sandbox has no GTK dev
libraries and `apt-get install libgtk-3-dev ...` failed here (package
mirror 404s) — an environment limitation, disclosed rather than
silently worked around. To still get real compiler verification of the
new `hierarchy.rs` (rather than only reading it), its exact content was
built as a standalone scratch crate against the real `platform-kernel`/
`api-server`/`async-trait`/`tokio`/`reqwest`/`serde`/`thiserror`
dependencies (no Tauri/GTK involved) — this caught one real issue (an
unused `Serialize` import; `HierarchyUserWire` only derives
`Deserialize`) and confirmed the file's 7 unit tests
(direct-manager-authorized, unrelated-user-denied, admin-always-
authorized, owner-not-self-authorized, unknown-actor-denied,
unknown-owner-denied, refresh-replaces-not-merges) all pass against the
real compiler. `desktop-shell/src/lib.rs`'s own diff (wiring
`HierarchyCache` into `login`/`logout`/`build_app_state`/`setup`) was
reviewed by hand against the actual, current file content and existing
patterns in that file (the same `tauri::State<T>` idiom already used for
`session`/`storage`/`local_replica`) — not compiled. This is a real,
disclosed verification gap: `desktop-shell` should be built and its
approval flow exercised against a running `api-server` on a machine
with the Tauri/GTK toolchain before this is considered fully proven end
to end, even though every piece that *could* be compiled here was.

**Verification actually run:**
- `cargo check --workspace --exclude desktop-shell --exclude
  admin-shell` — clean (the two excluded crates are the only ones
  requiring GTK; every other crate in the workspace, including every
  crate this fix touches, is covered).
- `cargo clippy --workspace --exclude desktop-shell --exclude
  admin-shell --all-targets --no-deps -- -D warnings` — clean, matching
  this repo's actual CI gate (`ci.yml`) exactly except for the two
  GTK-blocked crates.
- `cargo test -p platform-contracts -p work-domain -p mission-domain -p
  client-composition -p mobile-core` — all pass, including the full
  pre-existing `work-domain`/`mission-domain` unit suites (65 and 61
  tests respectively — no regression in any existing Task/Mission
  transition test), `app_state_wiring` (9), `task_end_to_end_sqlite`
  (2), `mission_end_to_end_sqlite` (2), and `mobile-core`'s
  `ffi_integration` suite (9, exercising the exact `AppState::new` call
  this session's earlier `mobile-core` fix touches).
- New end-to-end test added and passing:
  `client-composition/tests/task_owner_authority_gate.rs` — a real
  dispatch through `CommandRegistry`/`handle_command` against a live
  SQLite database (not a unit test of `HierarchyCache`'s logic in
  isolation): creates a task, has the owner mark it ready/start it/
  submit completion (confirms this last step still succeeds ungated),
  then confirms an unrelated stranger's `ApproveTask` is rejected with
  `CommandError::OwnerAuthorityDenied` specifically (not some other
  failure), and that the task's real, authority-resolved direct manager
  can approve it, advancing the persisted status to `Approved`.
- `cargo test -p api-server` has 6 pre-existing failing integration
  test binaries (`query_id_normalization`,
  `staff_loan_authorization`, `staff_profile_routes`,
  `team_leader_precheck_authorization`, `user_hierarchy_admin_routes`,
  `relay_switchboard` — all HTTP-integration tests that spin up a real
  server and log in) — confirmed, by stashing every change in this
  entry and re-running the identical `cargo test` commands against
  unmodified `main`, that every one of these fails identically on `main`
  with no changes from this session at all. This is a pre-existing
  environment issue in this sandbox (most failures are `access_token in
  login response` panics, suggesting the login round-trip — likely
  Argon2 password hashing — is too slow or otherwise failing under this
  sandbox's resource constraints), not something this fix caused or
  should be scoped to fix. `api-server`'s own unit tests (0, this crate
  has none) and the one `query_id_normalization` sub-test unrelated to
  login (`object_id_byte_array_converts_to_matching_uuid_string`, etc.)
  pass; the login-dependent ones do not, on `main` or with this fix
  applied.

**Remaining gap surfaced by this session's own verification, not
silently left implicit:** `desktop-shell` itself — the actual consumer
of `HierarchyCache`, `login`/`logout` wiring, and the Tauri commands a
real user would exercise — was never compiled, run, or clicked through
in this environment. Everything gating logic-level (`command_handler.rs`,
`decision_handler.rs`, the domain aggregates, `hierarchy.rs` itself) is
real-compiler-verified; the last mile connecting it into the actual
desktop app is reviewed-by-hand only. Build and manually exercise
`desktop-shell` (approve as a real logged-in manager; attempt to approve
as an unrelated user) on a machine with the Tauri/GTK toolchain before
treating this as fully proven.

## Release build matrix scoped: server Windows-only, desktop apps stay cross-platform — verified 2026-08-26

**The requirement, confirmed directly with the project owner.** Server
binaries (`api-server`/`worker`/`migration-tool`/`sync-agent`) only need
to run on Windows — the real production deployment is a Windows 10
machine in the office, and there is no current plan to run the server on
Linux or macOS. Both desktop apps (`desktop-shell`/"ONYX" and
`admin-shell`/"ONYX Admin") need to keep shipping on all three platforms
— Windows, macOS, and Debian Linux — since staff and admins use varied
hardware.

**What `release.yml` actually looked like before this — checked, not
assumed.** `release-binaries` built the four server binaries for all
three OSes (`ubuntu-24.04`/`x86_64-unknown-linux-gnu`,
`macos-14`/`aarch64-apple-darwin`, `windows-2022`/`x86_64-pc-windows-msvc`)
— more than the real deployment target needed.
`release-desktop`/`release-admin-desktop` already built all three real
targets (Linux `deb`+`appimage`, macOS `dmg`, Windows `msi`+`nsis`) —
no OS was missing from either matrix. `release-images` (GHCR/Docker) was
never OS-matrixed at all — one `ubuntu-24.04` runner builds every
service's Linux container image regardless of native-binary platform
support, since a Docker image is Linux-based by construction; it was
correctly left untouched.

**Real gap in the existing matrix, not one the task described.** Both
desktop jobs' macOS coverage was `aarch64-apple-darwin` only — no
`x86_64-apple-darwin` (Intel). This was raised explicitly via
`AskUserQuestion` rather than assumed either way, and the person
answering this session confirmed **Apple Silicon only** — no Intel Mac
build added. Disclosed plainly: that confirmation came from whoever was
in this chat session, not independently verified to be the same person
as "the project owner" referenced elsewhere in this task's brief; if
those are different people, this specific call should still be checked
with whoever actually owns it.

**The one matrix change made:** `release-binaries`'s `strategy.matrix`
narrowed from three entries to one — `{os: windows-2022, target:
x86_64-pc-windows-msvc}` — with a comment explaining why and why
`release-images` is deliberately untouched.

**Three real, unrelated bugs found and fixed via actual GitHub Actions
runs, not by reading the YAML and assuming it worked.** This job's own
existing comments claimed `apple-actions/import-codesign-certs`
ad-hoc-signs and skips notarization when no real Apple credentials are
configured, matching "Tauri's own documented fallback" — that claim had
never actually been exercised end to end before this session. It was
wrong in two different, specific ways:

1. **Icon bundling failure (both desktop apps' macOS builds).** First
   real run (`32718188664`) failed at
   `failed to bundle project: Failed to create app icon: Format error
   decoding Ico: The PNG is not in RGBA format!`. `desktop-shell`'s and
   `admin-shell`'s `icons/icon.ico` each had embedded PNG frames that
   decoded as RGB, not RGBA — Tauri's macOS bundler couldn't derive an
   `.icns` from them. Windows/Linux builds were unaffected since neither
   path decodes the `.ico` into an `.icns`. Fixed by regenerating both
   apps' `icon.ico` (now genuine RGBA per frame, confirmed with `file`)
   and generating a proper `icon.icns` via `tauri icon icons/icon.png`
   against each app's existing source image — no new artwork. That
   command also generates Android/iOS/MSIX assets neither app uses (no
   mobile Tauri target, no Windows Store packaging); those were deleted
   rather than committed, keeping `icons/` to exactly `icon.ico`,
   `icon.png`, `icon.icns`. Added `icons/icon.icns` to both
   `tauri.conf.json`'s `bundle.icon` list so macOS bundles from the
   dedicated, higher-resolution `.icns` instead of re-deriving a
   lower-res one from `icon.ico` (which tops out at a 256×256 frame).

2. **Empty-string codesign identity (both apps' macOS builds).** Second
   run (`32906999058`), after the icon fix, got past icon bundling and
   failed instead with `Signing with identity ""` /
   `error: The specified item could not be found in the keychain` /
   `failed codesign application`. Root cause:
   `${{ secrets.APPLE_SIGNING_IDENTITY }}` evaluates to an empty
   string, not unset, when that secret doesn't exist (it doesn't — no
   real Apple signing credentials are configured for this repo yet).
   Tauri's bundler treats an explicitly-set-but-empty
   `APPLE_SIGNING_IDENTITY` differently from a genuinely absent one: it
   attempts `codesign` with identity `""` instead of falling back to
   ad-hoc `"-"` signing. Fixed with
   `${{ secrets.APPLE_SIGNING_IDENTITY || '-' }}` on both jobs — falls
   back to ad-hoc only when the secret is genuinely unset; a real
   configured identity still wins.

3. **Empty-credential notarization attempt (both apps' macOS builds).**
   Third run (`32906999058`, same run as #2's fix landed for), after the
   codesign fix, got past codesign (`Signing with identity "-"`
   succeeded) and failed instead at notarization:
   `Error: Team ID must be at least 3 characters`. Same root pattern as
   #2, but `env: X || 'fallback'` can't fix it the same way this time —
   there is no valid non-empty fallback value for a real Apple ID/team
   the way `-` is a valid signing identity, and `${{ secrets.APPLE_ID }}`
   etc. are still empty strings, not unset, so Tauri's bundler treated
   "configured" as true and attempted notarization anyway. GitHub
   Actions' `env:` mapping has no expression syntax to conditionally
   omit a key. Fixed with a new "Configure macOS notarization
   environment" step on both jobs, run before the `tauri-action` step,
   that exports `APPLE_ID`/`APPLE_PASSWORD`/`APPLE_TEAM_ID` to
   `$GITHUB_ENV` only when all three secrets are genuinely non-empty;
   the three keys were removed from `tauri-action`'s own `env:` block
   since `$GITHUB_ENV` already propagates them into the job's real
   process environment when configured, and a truly-absent var there is
   what correctly triggers Tauri's skip-notarization path (confirmed:
   `tauri-action`'s bundler reads real process env vars via Rust's
   `env::var`, which distinguishes absent from empty; GitHub Actions'
   `env:` mapping cannot).

**Verification actually run — the real, final green run.** Run
`33003707428` (commit `de31739`) completed with every job green:
`release-binaries` (Windows only) succeeded; `release-desktop` and
`release-admin-desktop` each succeeded on all three platforms. Confirmed
via `actions_list list_workflow_run_artifacts`, not assumed from the
green checkmarks alone — all 7 expected artifacts present with real,
non-trivial sizes:
- `binaries-x86_64-pc-windows-msvc` (server, ~19 MB)
- `desktop-x86_64-pc-windows-msvc` (ONYX Windows, ~15 MB)
- `desktop-x86_64-unknown-linux-gnu` (ONYX Linux `.deb`+`.AppImage`, ~95 MB)
- `desktop-aarch64-apple-darwin` (ONYX macOS `.dmg`, ~10 MB)
- `admin-desktop-x86_64-pc-windows-msvc` (ONYX Admin Windows, ~6 MB)
- `admin-desktop-x86_64-unknown-linux-gnu` (ONYX Admin Linux, ~81 MB)
- `admin-desktop-aarch64-apple-darwin` (ONYX Admin macOS `.dmg`, ~5 MB)

Each of the three bugs above was diagnosed from the actual job log text
of a real failing run (via `mcp__github__get_job_logs`), fixed, pushed,
and re-verified with a fresh `workflow_dispatch` run before moving to
the next — four real runs total across this effort
(`32718188664` → `32906999058` → `33003707428` succeeding), not one
fix-and-hope pass.

**What this does NOT cover.** Real, working code-signing/notarization
for macOS (a real Apple Developer certificate + team ID) is still not
configured for this repo — every macOS build in this matrix is
ad-hoc-signed and unnotarized, which is fine for internal builds but
would show Gatekeeper warnings for any real external distribution. That
was already true before this session and is unchanged; flagged here so
it isn't mistaken for having been fixed. Windows code-signing (a real
Authenticode certificate) is likewise still unconfigured — GPG-signing
of the release tarball/bundles only happens on a real tag push
(`github.event_name == 'push'`), not on the `workflow_dispatch` runs
used for this verification, so that path was not exercised here either.

## Mobile Task/Mission approval authority — Rust/FFI layer closed, real gap found blocking the rest

**The gap this closes.** `mobile-core`'s `mobile_core_new` passed
`owner_authority: None` to `AppStateConfig`, which resolves to
`client_composition::DenyAllOwnerAuthority` — every `ApproveTask`/
`RejectTask`/`RejectApproval`/`ActivateMission` on mobile was denied
unconditionally, for everyone, including a real manager (fail-closed,
but non-functional). Confirmed exactly as a same-session planning
document described before touching anything.

**Design decisions, made explicitly, not assumed:**

1. **FFI data hand-off: a separate `mobile_core_set_hierarchy(handle,
   hierarchy_json)` call, not baked into `mobile_core_new`'s config.**
   `mobile_core_new` must succeed (opening the local SQLite pool,
   applying migrations) before any network call could possibly happen,
   and login/hierarchy-fetch is a genuinely separate-timing event from
   local-database bootstrap. A separate call keeps those two timelines
   independent, matching the plan's own reasoning.

2. **`HierarchyCache` splitting: moved to `client-composition`, not
   split in place inside `desktop-shell`.** The plan posed this as
   options (a) (extract the "replace from parsed data" logic into a
   shared method, `refresh()` becomes a thin wrapper) vs. (b) (move the
   whole type to a shared crate). On inspecting the actual dependency
   graph, both options converge on the same real requirement neither
   framing stated explicitly: `desktop-shell` is a **binary** crate, and
   a binary cannot be a library dependency of `mobile-core` — the type
   had to move to somewhere both binaries already depend on regardless.
   `client-composition` is that place (it already houses
   `AppState`/`DenyAllOwnerAuthority`, and both `desktop-shell` and
   `mobile-core` already depend on it). Moved the whole `HierarchyCache`
   type there (new module `client_composition::hierarchy_cache`,
   `reqwest` added to that crate's dependencies), with `refresh()`
   (HTTP-fetching, `desktop-shell`'s path) and a new
   `load_from_json`/`replace_from_wire` split (JSON-only,
   `mobile-core`'s path) sharing one real implementation of the id-
   parsing/map-building logic — matching option (a)'s actual intent,
   just correctly scoped to where the type needed to live. `desktop-
   shell`'s own `hierarchy.rs` was deleted; its `lib.rs` now references
   `client_composition::hierarchy_cache::HierarchyCache` directly. All 6
   of that module's original unit tests carried over unmodified, plus a
   new 7th (`load_from_json_populates_cache_from_the_same_wire_shape_
   the_server_sends`) covering the new mobile-only data path.

3. **`AppState` construction: interior-mutable cache handed in at
   construction, not a `RwLock<Arc<AppState>>` rebuild.** The plan
   flagged this as its own likely-correct answer and asked for
   confirmation before assuming it. Confirmed: `mobile_core_new`
   constructs one `HierarchyCache`, hands `Some(Arc::new(cache.clone())
   as Arc<dyn OwnerAuthority>)` into `AppStateConfig.owner_authority` at
   construction (not `None`), and keeps the same `HierarchyCache` on
   `MobileApp` so the new `mobile_core_set_hierarchy` FFI function can
   populate it later, in place. Because `HierarchyCache`'s internal
   `Arc<RwLock<HashMap<...>>>` is the same shared instance `AppState`
   already holds a reference to through `owner_authority`, populating it
   after the fact is immediately visible to every subsequent authority
   check — no `AppState` rebuild, no `MobileApp` handle-lifecycle change
   at all.

**Real, unanticipated gap found — flagged, not silently worked around or
built past.** The plan's Step 1 ("fetch the hierarchy on the Dart side
using the same `dio` client `OnyxHttpAuthApi` already authenticates,
right after login") assumed an authenticated HTTP session exists in the
same code path that constructs `mobile-core`'s `AppState`. Checked
directly against `mobile/lib/main.dart`: **it does not.** `OnyxMobile`
(the FFI transport, the one `owner_authority` actually lives on) and
`OnyxHttpApi` (the HTTP transport, which has real login via
`OnyxHttpAuthApi`) are two independent, mutually exclusive
implementations of `OnyxApi` — `main.dart` picks exactly one per launch
based on a saved `transport_mode` preference. FFI mode's
`_initializeMobileCore` has **no login step of any kind**:
`organization_id`/`user_id` come straight from `SharedPreferences`
(defaulting to hardcoded placeholder UUIDs), with no server round-trip,
no token, nothing to fetch `GET /api/users/hierarchy` with. `OnyxHttpApi`
does have real login, but it has no local `mobile-core` `AppState` at
all — every command goes straight to `api-server` over HTTP, which
already enforces owner-authority server-side (this session's earlier
`4ae6091` fix) — so it was never affected by this bug and has no local
cache to populate either way. The mechanism this session built (the
Rust FFI function, the shared cache, `OnyxApi.setHierarchy` on the
interface, `OnyxMobile`'s real implementation via the new
`mobile_core_set_hierarchy` FFI binding, `OnyxHttpApi`'s honest no-op)
is real and independently tested, but **it has no call site that can
actually run in production today** — FFI-mode mobile has no
authentication mechanism to fetch a hierarchy with in the first place.
Approvals therefore remains non-functional on mobile in practice (still
fails closed, correctly, not open) until FFI-mode mobile gains some real
login/auth mechanism — a separate, larger, unplanned piece of work this
session deliberately did not start, per this project's own "don't
silently expand scope" rule. A Dart-side hierarchy-fetch method was
deliberately NOT added with no real caller, since it would be
untestable dead code; that piece should be built together with whatever
FFI-mode auth mechanism eventually exists to call it from.

**Step 5 (Approvals screen's stale empty-state text) — checked, found
to not apply, left untouched.** The plan assumed
`mobile/lib/ui/screens/approvals.dart`'s "No local Approval aggregate is
registered" message was about Task/Mission approval and would go stale
once this fix landed. Checked directly: that screen's `controller.
approvals` is `listAggregates('approval')` — a **different, unrelated
aggregate type** (`ApprovalAggregate`, the same one referenced in
`decision_handler.rs`'s list of aggregates with no owner concept — staff-
loan/list-verification approvals, not Task/Mission `ApproveTask`).
Confirmed no `"approval"` repository is registered anywhere in
`client-composition::app_state` at all, on any client — the message is
still completely accurate and was left unmodified. Task/Mission's own
approve/reject state surfaces through the existing Missions/Tasks
screens' own `status` field becoming `Approved`/`Rejected`, not through
this screen. Whether `ApprovalAggregate` should ever be registered
locally on mobile/desktop is a separate, real gap this session did not
investigate further — flagged as a possible future task, not acted on.

**Verification actually run:**
- `cargo check`/`cargo clippy --workspace --exclude desktop-shell
  --exclude admin-shell --all-targets --no-deps -- -D warnings` — clean
  (the two excluded crates remain GTK-blocked in this sandbox, same
  limitation as this session's earlier release-matrix work; `mobile-core`
  and `client-composition` are both covered and both clean).
- `cargo test -p client-composition` — the moved `hierarchy_cache`
  module's full 8-test suite (7 original + 1 new) passes; no regression
  in the full `work-domain`/`mission-domain`/`platform-contracts` suites
  run alongside it.
- `cargo test -p mobile-core` — the existing 9-test `ffi_integration`
  suite still passes unmodified (no regression from adding
  `owner_authority: Some(...)`/the new FFI function), plus a new real
  end-to-end test, `hierarchy_authority_gate.rs`, through the actual
  `extern "C"` FFI boundary (not an in-process shortcut): confirms
  `ApproveTask` is denied for everyone before `mobile_core_set_hierarchy`
  is ever called (the fail-closed default), confirms a stranger is still
  denied after a real hierarchy is loaded, confirms the task's real
  cache-resolved direct manager can approve, confirms `SubmitCompletion`
  succeeds ungated throughout, and reloads via a fresh `GetTask` FFI
  query afterward to confirm the approval genuinely persisted (caught
  and fixed one real test-authoring bug in the process: the query
  result nests aggregate fields under `"aggregate"`, not at the top
  level — confirmed by printing the actual JSON rather than assuming
  the shape).
- Dart changes (`bridge.dart`'s new `setHierarchy`/FFI binding,
  `onyx_http_api.dart`'s no-op implementation) were hand-verified
  against the file's own existing patterns (exact `malloc.free`/FFI-
  typedef shapes already used for every other binding) but **not**
  compiled or analyzed — no Dart/Flutter SDK fits in this sandbox
  (same disclosed constraint as the plan document itself noted). Should
  be run through `dart analyze`/`flutter analyze` on a real toolchain
  before being considered fully verified, the same standard this
  project has applied to every other Dart change.

## Class-based mobile access control — restrictive by default, per explicit project-owner decision

**The gap.** Mobile login had no per-class access control at all: any
active user with correct credentials could log in from the mobile app,
regardless of their `UserClass`. This piece adds an admin-managed
allow-list, `mobile_class_access` (per-organization, per-class grant
rows), gating `client_type: "mobile"` logins specifically.

**The one question this piece required asking before writing any code**
(per the plan's own explicit instruction — "don't default to the plan's
tentative recommendation without an explicit go-ahead"): for an
organization with **no** `mobile_class_access` rows configured at all,
should mobile login be allowed (permissive) or denied (restrictive)?
This was asked directly via `AskUserQuestion`, not inferred or defaulted.
**Answer received: restrictive.** An org with zero grant rows denies
mobile login for every class until an admin explicitly adds one. This is
the opposite of the plan document's own tentative "permissive" lean —
implemented exactly as answered, not as the plan first suggested.

**What was built:**

- `migrations/{postgres,sqlite}/20260108000000_add_mobile_class_access.{up,down}.sql`
  — one row per `(organization_id, user_class)` grant, `UNIQUE`
  constraint preventing duplicates, mirroring the existing
  `20260107000000_add_user_class_hierarchy` migration's own
  documentation conventions (each file explains its own provenance and
  the Postgres/SQLite validation-strength gap, same as that precedent).
- `UserStore` port (`security-application`): two new methods,
  `list_mobile_access`/`set_mobile_access` (the latter replaces the
  full grant set atomically inside one transaction — a partial
  read mid-write must never be observable), implemented for both
  `PostgresUserStore` and `SqliteUserStore`.
- `LoginRequest` (`api-server::routes::auth`) gains an additive,
  optional `client_type: Option<String>` field. Only
  `client_type == Some("mobile")` triggers the gate — anything else
  (including `None`, for any caller this project doesn't yet know
  about) is never gated, so this cannot silently lock out a caller
  that predates the field. **Admin bypasses the gate unconditionally**
  regardless of `client_type`, mirroring the existing `require_class`
  precedent elsewhere in `admin.rs` where Admin bypasses every
  class-based check in this codebase — this was not asked about
  separately since it is consistent with that established pattern, not
  a new judgment call. An unclassified user (`class: None`) can never
  match a grant row (`user_class` is `NOT NULL`), so is denied by
  construction, consistent with the restrictive-default answer above.
  Denial returns a new, specific `403 MOBILE_ACCESS_RESTRICTED` — kept
  distinct from `invalid_credentials()`'s deliberately-generic
  `401 INVALID_CREDENTIALS` (audit finding H-01's enumeration-resistance
  requirement), since a mobile-access denial is disclosed only *after*
  a real credential match and is not a credential-guessing surface.
- New admin-only routes, `GET`/`PUT /api/admin/mobile-access`, scoped to
  the calling admin's own organization (no cross-org admin concept
  exists anywhere else in `admin.rs` either, so this follows that same
  precedent rather than accepting an arbitrary `organization_id`).
- `admin-shell`'s Settings page gained a `MobileAccessPanel` — a
  checkbox per `UserClass`, reading/writing the new endpoints directly
  via `apiClient` (this is a plain table, not an event-sourced
  aggregate, so it does not go through `useCommand`/`useQuery` — same
  reasoning `ServerConnectionSettings`/`Profiles.tsx` already use for
  their own plain-table settings).
- Every first-party login call site was updated to send its own real
  `client_type` for consistency, even where the value doesn't trigger
  the gate: `desktop-shell` sends `"desktop"`, `admin-shell` sends
  `"admin"`, `mobile`'s `net/auth.dart` sends `"mobile"`.
- `mobile/lib/net/auth.dart` gains `MobileAccessRestrictedException`,
  thrown specifically when the server's login rejection carries
  `MOBILE_ACCESS_RESTRICTED` — distinguished from every other login
  failure, which still surfaces as the deliberately-generic
  `INVALID_CREDENTIALS` per that file's own existing doc comment (audit
  finding H-01). `http_login_screen.dart`'s `_friendlyLoginError` shows
  a specific "ask your admin to enable mobile access" message for this
  one case rather than the generic "sign-in failed" text.

**Verification actually run:**
- `cargo check --workspace --exclude desktop-shell --exclude admin-shell`
  and `cargo clippy` (same exclusions, `-D warnings`) — clean.
  `desktop-shell` itself was independently confirmed to still hit the
  same pre-existing, unrelated `gdk-3.0` pkg-config/GTK limitation this
  sandbox has hit throughout this session (not a new failure from this
  change — confirmed by reading the actual error text again).
- `cargo test -p security-adapter` — 25/25 pass, including a new test,
  `mobile_access_defaults_to_empty_and_replaces_wholesale`, proving the
  restrictive default (`list_mobile_access` on an unconfigured org
  returns empty, not "everyone"), per-org isolation, and that
  `set_mobile_access` truly replaces rather than merges.
- `cargo test -p api-server --test mobile_access_gate` — a new, real,
  full-HTTP end-to-end test: an Admin can log in with
  `client_type: "mobile"` before any grant exists (bypass confirmed); a
  Staff user with the same client_type is denied
  `403 MOBILE_ACCESS_RESTRICTED` before any grant exists (restrictive
  default confirmed over real HTTP, not just at the store layer); the
  same Staff user succeeds with `client_type: "desktop"` (gate is
  `client_type`-scoped, confirmed); `GET /api/admin/mobile-access`
  starts empty; `PUT` grants `"staff"`; the Staff user's mobile login
  then succeeds; and a non-admin's own `GET` on the admin route is
  confirmed `403`. All assertions pass.
- Ran the **full** `api-server` test suite alongside this change and
  found four pre-existing failures unrelated to this work — confirmed
  by re-running each on unmodified `main` (via `git stash`) and getting
  the identical failure count and messages there too:
  `query_id_normalization` (the seeded fixed `"All-Father"` admin
  account, added in a prior session, means the token-gated
  `/api/admin/bootstrap` flow that test relies on is always
  `BOOTSTRAP_ALREADY_COMPLETED` against a fresh database — the new
  `mobile_access_gate` test deliberately logs in as the seeded admin
  instead, precisely to avoid this trap), `relay_switchboard` (3
  failures, appear to be a WebSocket/port-binding limitation in this
  sandbox), and `staff_loan_authorization` /
  `user_hierarchy_admin_routes` / `staff_profile_routes` /
  `team_leader_precheck_authorization` (8 total failures, same
  bootstrap-vs-seeded-admin conflict). None of these were introduced or
  worsened by this change, and none were touched — fixing pre-existing,
  unrelated test failures was out of scope for this task.
- `admin-shell/ui`: `npx tsc -b` and `npx vite build` both clean (a
  working Node toolchain was available in this sandbox for this piece,
  unlike the Dart/Flutter case above).
- Not verified: an actual live-server, real-browser click-through of
  the new `MobileAccessPanel` UI (no running `api-server` + browser
  session was exercised, only the build); a real Android/iOS mobile
  build (no Flutter SDK in this sandbox, same constraint as Piece 1).

## Mobile file sharing — FFI layer and UI closed; HTTP-transport upload confirmed out of scope

**Pre-implementation confirmation, per the plan's explicit requirement.**
Before writing any code, checked whether `api-server` already has an
HTTP file upload/download route (the plan flagged this as unconfirmed).
Read `crates/bins/api-server/src/routes/` directly: the only multipart
handling anywhere is `routes/profiles/batch.rs`'s CSV batch-import
endpoint, which is unrelated (staff-profile bulk import, not general
file storage). There is no HTTP route reaching
`FileUploadCoordinator`/`BlobStore` at all — confirmed absent, not
assumed. This means `OnyxHttpApi` (the HTTP transport) has no backend
to call even if a Dart method were added for it; building that backend
route was new, unplanned server work outside this piece's scope, so the
HTTP transport's `uploadFile`/`downloadFile` throw a clear
`UnsupportedError` explaining exactly why, rather than silently
no-op'ing (which would look like success to a caller) or faking a
route that doesn't exist.

**What was built (FFI / local-first transport):**
- `mobile-core/src/ffi_files.rs`: `mobile_core_upload_file`/
  `mobile_core_download_file`, mirroring `desktop-shell`'s
  `upload_file`/`download_file` Tauri commands exactly — both sit on
  the same shared `client_composition::file_upload::FileUploadCoordinator`
  via their own `AppState`. Takes a filesystem **path** in (not raw
  bytes), for the same IPC-cost reason desktop-shell's own doc comment
  gives: shipping up to a 100 MB file through an FFI/IPC boundary as an
  argument would cost a full extra copy the native side reading the
  file directly avoids. MIME type is hardcoded to
  `"application/octet-stream"`, matching desktop-shell's own documented
  choice (no MIME-sniffing library is a workspace dependency; guessing
  from the extension would be a half-measure).
  `mobile_core_download_file` returns bytes-written as `i64` with `-1`
  as the failure sentinel (adapting desktop's `Result`-based contract
  to C-ABI, which has no `Result`).
- `mobile-core.h` auto-regenerated via the existing `build.rs`/cbindgen
  pipeline — confirmed both new function signatures appear.
- `mobile/lib/bridge/bridge.dart`: `OnyxApi` interface gains
  `uploadFile`/`downloadFile`; `OnyxMobile` implements them via new FFI
  typedefs/bindings, `OnyxHttpApi` throws the explicit
  `UnsupportedError` described above.
- `mobile/lib/ui/screens/files.dart`: new screen, added to the bottom
  navigation between Approvals and Settings. Takes a filesystem path
  via a plain `TextField` rather than integrating a file-picker
  package: no such dependency exists in `pubspec.yaml` today, and this
  sandbox has no Flutter/Dart toolchain to verify a new native
  dependency actually builds on a real device — adding one unverified
  would be a larger, riskier, unplanned change than this screen itself.
  A real file-picker UI is a natural, flagged follow-up once it can be
  built and tested on a real device or CI.
- **Bug caught while adding this piece, unrelated to file sharing
  itself:** `mobile/test/fakes.dart`'s `FakeOnyxApi` was already missing
  the `setHierarchy` override Piece 1 added to the `OnyxApi` interface
  — a real gap from that earlier piece that went uncaught because this
  sandbox has no Dart toolchain to run `dart analyze` and catch a
  missing `@override`. Fixed here (added `setHierarchy` alongside the
  new `uploadFile`/`downloadFile` overrides), disclosed rather than
  silently folded in as if it were always part of this piece.

**Verification actually run:**
- `cargo check -p mobile-core` and `cargo clippy -p mobile-core
  --all-targets --no-deps -- -D warnings` — clean.
- `cargo test -p mobile-core --test file_sharing` — two new real,
  end-to-end tests through the actual FFI boundary:
  `upload_then_download_round_trips_byte_for_byte_through_real_ffi`
  (uploads a real 10,000-byte file from disk, downloads it back by its
  returned content hash, and asserts the downloaded bytes match the
  original exactly) and
  `upload_rejects_a_file_exceeding_the_max_size_with_a_clear_failure`
  (a real 100 MiB + 1 byte file on disk is rejected — confirmed the
  domain-level `MAX_FILE_SIZE_BYTES` check, already enforced inside
  `file-domain`'s aggregate and shared by both clients through
  `FileUploadCoordinator`, needed no duplicate check added here). Both
  pass.
- Dart changes (`bridge.dart`'s new typedefs/bindings/implementations,
  `onyx_http_api.dart`'s explicit-failure implementations,
  `files.dart`'s new screen, `fakes.dart`'s fix) were hand-verified
  against this file's own existing patterns but **not** compiled,
  analyzed, or run — no Dart/Flutter SDK fits in this sandbox, the same
  disclosed constraint as Pieces 1 and 2's Dart work. Should be run
  through `dart analyze`/`flutter test` on a real toolchain before
  being considered fully verified.
- Not verified: a real Android/iOS build; an actual click-through of
  the new Files screen in a running app.

## Correction to the class-based mobile access control piece: `web-ui` had been missed

Re-verifying that piece on direct request surfaced a real gap in the
original report: it claimed every first-party client sends
`client_type`, but `web-ui` (`crates/bins/web-ui` was checked; the real
app lives at the repo-root `web-ui/`) was never actually checked and
did not send it — `web-ui/src/hooks/useAuth.ts`'s `login` mutation
posted only `{username, password}`. Fixed by adding
`client_type: "web"` there. This has no behavioral effect today (only
`client_type: "mobile"` is ever gated), but the original claim of
"every client" was inaccurate until this fix — recorded here rather
than left silently corrected.

Also strengthened `mobile_access_gate.rs` with the specific scenario
requested when this piece was re-verified: a new test,
`excluded_class_denied_on_mobile_allowed_on_desktop_granted_class_allowed_on_both`,
grants only `"supervisor"` up front (never `"staff"`) and confirms
`staff` is denied on `client_type: "mobile"` but succeeds on
`client_type: "desktop"`, while `supervisor` succeeds on both. Real run:
both tests in that file pass (`cargo test -p api-server --test
mobile_access_gate` — 2 passed, 0 failed).

`web-ui`'s `tsc -b` and `vite build` both re-run clean after the fix.

## Real login for FFI-mode mobile — closes the placeholder-identity gap flagged after Piece 1

**The gap.** Every prior mobile session in this project confirmed and
disclosed the same thing: FFI-mode mobile (`main.dart`'s `restartApp`,
`transport_mode != 'http'`) had no login step at all.
`organization_id`/`user_id` came straight from
`SharedPreferences.getString(...) ?? <hardcoded placeholder UUID>`
(`'11111111-1111-1111-1111-111111111111'` /
`'33333333-3333-4333-8333-333333333333'`), with zero server round-trip.
This closes that gap.

**Design decision, checked against the real current shape of
`mobile_core_new`/`MobileConfig` first, not assumed** (confirmed by
reading `crates/mobile-core/src/lib.rs` and
`mobile/lib/bridge/bridge.dart`'s `MobileCoreConfig` directly): identity
resolution stays entirely in Dart; **no changes to `mobile_core_new`'s
FFI signature were needed.** Two things made this the clear choice, not
a coin flip:

1. `mobile_core_new` already takes `organization_id` as a plain,
   client-supplied config value at construction, with no auth step of
   its own — so there was nothing to "remove" from it to make room for
   a login step; Dart already has to know the org id before calling it
   either way.
2. `mobile-core`'s Rust side has **no working `SecureStorage`
   implementation at all** — confirmed by reading
   `crates/mobile-core/src/ffi_secure_storage.rs` directly: genuinely
   blocked on real JNI (Android Keystore) / Objective-C (iOS Keychain)
   bridge code this sandbox cannot write or verify, same category of
   gap as `mobile-core`'s missing MIME-sniffing library. A Rust-side
   `mobile_core_login` that performs its own HTTP call would need
   exactly that missing mechanism to persist tokens safely — building
   it now would mean building two new, unverifiable things (a second
   HTTP client duplicating `net/auth.dart`'s already-tested login logic,
   plus a secure-storage bridge already known to be out of reach here)
   instead of reusing one that already works.

So this piece is **pure Dart, zero Rust crate changes** — confirmed by
`git status` showing nothing under `crates/` touched. The shape mirrors
Piece 1's own precedent exactly: fetch/decide in Dart
(`OnyxHttpAuthApi`, already built and tested for HTTP-mode), hand the
*result* to Rust as plain data via the existing FFI surface
(`mobile_core_set_hierarchy`, unchanged) — never re-implement a second
Rust-side HTTP client for something Dart already does correctly.

**What was built:**

- `mobile/lib/ui/ffi_login_screen.dart` (new) — real login screen shown
  by `main.dart::restartApp` whenever no real session exists yet
  (`SharedPreferences`'s new `ffi_session.has_real_session` flag is
  unset). Performs a real `POST /api/auth/login`
  (`client_type: "mobile"`) via the *same* `OnyxHttpAuthApi` HTTP-mode
  already uses (`net/auth.dart` — reused, not duplicated), persists the
  real `organization_id`/`user_id`/`username` (non-secret,
  `SharedPreferences`) and the real access/refresh tokens (secret,
  `FfiSessionStorage` — see below), opens mobile-core for real under the
  real `organization_id` via `initializeFfiMobileCore`, and
  best-effort-fetches the org's hierarchy and loads it via the existing
  `OnyxApi.setHierarchy` — mirroring `desktop-shell::login`'s own
  "best-effort, logged not propagated as a login failure" handling of
  the identical step.
- `mobile/lib/net/auth.dart` gains `OnyxHttpAuthApi.fetchHierarchyJson()`
  — a thin `GET /api/users/hierarchy` wrapper returning the raw JSON
  string `setHierarchy` expects. This is the exact Dart-side hierarchy
  fetch Piece 1's own `DECISIONS.md` entry explicitly deferred building
  ("would be untestable dead code" with no real caller at the time) —
  it now has one.
- `mobile/lib/main.dart`: `restartApp` gates FFI mode on
  `hasRealFfiSessionKey`, routing to `FfiLoginScreen` when unset;
  `_initializeMobileCore` renamed to public `initializeFfiMobileCore`
  (reused by the login screen) and no longer falls back to the
  placeholder UUIDs — both removed entirely, along with the
  `defaultOrganizationId`/`defaultUserId` constants that supplied them.
  A new best-effort, **fire-and-forget** (`unawaited`, deliberately not
  blocking startup) hierarchy refresh runs on every successful open
  using the previously-saved access token, so a reopened app doesn't
  start every session with a cold, empty approval-authority cache.
- `mobile/lib/background/android/workmanager_service.dart`: the
  identical hardcoded-placeholder fallback existed here too (a second,
  separate reachable path this session's own mobile audits had not
  previously flagged) — the periodic background sync task now no-ops
  cleanly (returns success, does nothing) when no real session exists,
  instead of opening mobile-core under a fake organization.
- `mobile/lib/ui/screens/settings.dart` gains a "Sign out" action for
  FFI mode (parity with `desktop-shell`'s `logout` command): clears
  `FfiSessionStorage`, clears the persisted identity, and restarts the
  app back to `FfiLoginScreen`.
- **`SharedPreferences` was judged inadequate for the real tokens, and
  something more appropriate was used instead — stated plainly, not
  silently assumed adequate.** `mobile/lib/net/session_storage.dart`
  (new) adds `flutter_secure_storage` (Android Keystore-backed
  `EncryptedSharedPreferences` / iOS Keychain — the same class of
  OS-backed mechanism `desktop-shell`'s `SecureStorage` port already
  uses) specifically for the access/refresh tokens, while
  `organization_id`/`user_id`/`username` (non-secret facts, not
  credentials) remain in `SharedPreferences` as before. This is a new
  *native* dependency (bundles real Kotlin/Swift platform code) —
  flagged as a materially bigger unverified-in-this-sandbox risk than
  this project's other disclosed Dart gaps, for the same reason
  `ffi_secure_storage.rs` already discloses for mobile-core's Rust side:
  no Android/iOS toolchain here can build, link, or exercise real
  platform secure-storage code at all.

**A second, real, pre-existing gap found and disclosed while building
this (not fixed — separate, unplanned server work):** `api-server` has
**no `/api/auth/refresh` route anywhere** — confirmed by reading
`routes/auth.rs`/`routes/mod.rs` directly. `POST /api/auth/login` issues
a 1-hour access token and a 7-day refresh token, but nothing in this
codebase has ever built a way to redeem the refresh token for a new
access token. Consequence, stated plainly: the persisted FFI-mode
session lets the app reopen under the correct, real
`organization_id`/`user_id` indefinitely (those are stable facts), but
the best-effort hierarchy refresh above stops succeeding roughly an hour
after the last real login — at which point approvals correctly fail
closed (the existing, safe empty-cache default) until the next real
password login. This is a real limitation of "session persistence" here,
not the full parity with `desktop-shell` a working refresh route would
give; flagged rather than glossed over.

**A third, pre-existing gap, left deliberately untouched and disclosed
rather than silently fixed or silently ignored:**
`ui/screens/settings.dart`'s existing "Organization UUID"/"User UUID"
free-text fields (`OnyxController.saveSettings`) already let a person
manually override their local identity to *any* UUID, with no
connection to a real login at all — predates this piece, and this task
was scoped to startup identity resolution, not this settings affordance.
`ui/startup_error_screen.dart`'s manual recovery fields have the same
property. Neither was touched; both remain a real, if narrower, way to
run mobile-core under an identity nobody authenticated as. Not silently
folded into "no placeholder identity path remains reachable" — it does,
just not via a hardcoded default anymore.

**Verification actually run:**
- `git status` confirms zero files under `crates/` were touched by this
  piece — no `cargo check`/`cargo test` runs were needed or possible for
  a change that touched no Rust code; `cargo check --workspace
  --exclude desktop-shell --exclude admin-shell` was re-run anyway as a
  sanity check and remains clean (unsurprising, since nothing it covers
  changed).
- Dart changes (`ffi_login_screen.dart`, `session_storage.dart`,
  `main.dart`, `auth.dart`, `settings.dart`,
  `workmanager_service.dart`, `app.dart`/`startup_error_screen.dart`/
  `bridge.dart` doc-comment fixes) were hand-verified against this
  project's own existing patterns (mirroring `http_login_screen.dart`'s
  structure closely, reusing `OnyxHttpAuthApi`/`OnyxHttpClient`
  unchanged) and manually brace-balance-checked, but were **not**
  compiled, analyzed, or run — this sandbox has no Dart/Flutter SDK at
  all (`dart`/`flutter` both confirmed absent via `which`), the same
  disclosed constraint as every prior mobile piece this session. This is
  the largest Dart change in this project without compiler
  verification to date and carries correspondingly more risk than
  earlier, smaller pieces; it must be run through `flutter analyze` and
  a real device/emulator test before being trusted in production.
- No `OnyxApi` interface methods were added or changed, so
  `mobile/test/fakes.dart`'s `FakeOnyxApi` needed no update this time
  (checked directly — confirmed, not assumed, since Piece 3 already
  found one real miss there).
- Confirmed no existing Dart test imports `main.dart` or exercises
  `restartApp`/`FfiLoginScreen`/`OnyxControllerHost`'s construction
  path directly, so none of the existing test suite's expectations were
  contradicted by these changes (all existing tests construct
  `OnyxController` directly against fixed literal test UUIDs,
  independent of `main.dart` entirely).

**Explicit confirmation on remaining placeholder identity paths, per
this project's own established standard of catching and disclosing
this precisely (see the Piece 2 `web-ui` correction):** the hardcoded
placeholder UUIDs (`defaultOrganizationId`/`defaultUserId`) and their
use as a silent fallback are removed from every code path that
previously reached them — `main.dart::restartApp`,
`initializeFfiMobileCore`, and `workmanager_service.dart`'s background
task. **However, "done" does not mean no path remains that can run
mobile-core under an unauthenticated identity at all** — the two
pre-existing manual-override surfaces named above
(`settings.dart`'s identity fields, `startup_error_screen.dart`'s
recovery fields) still let someone type in arbitrary UUIDs by hand,
untouched by this piece and out of its stated scope. Say so plainly
rather than let the hardcoded-default fix stand in for a broader claim
it doesn't cover.

## Fixed: manual organization/user UUID entry was a real security hole, not a rough edge

Immediately after the previous entry disclosed it, the project owner
correctly classified this as a security hole, not a follow-up nicety —
a manual UUID-entry path that bypasses login entirely defeats the whole
point of real login/approval-authority gating just built: anyone with
the app installed could type in someone else's real
`organization_id`/`user_id` and have mobile-core act as them, no
credentials required. Fixed immediately, ahead of the lower-severity
token-refresh gap.

**Two reachable instances, both closed:**

1. `ui/screens/settings.dart` — the "Organization UUID"/"User UUID"
   `TextField`s, saved via `OnyxController.saveSettings(organization:
   ..., user: ...)`. Removed entirely, not merely hidden: `saveSettings`
   itself no longer accepts an `organization`/`user` override at
   all — replaced with `saveRelayEndpoint(String relay)`, which only
   ever touches the Cloud Relay endpoint. This closes the hole at the
   method level, not just the widget tree, so no other future caller of
   `OnyxController` could reach it either. `organization_id`/`user_id`
   are now shown **read-only** in this screen, with a pointer to the
   new "Sign out" action for actually changing identity.
2. `ui/startup_error_screen.dart` — the identical pattern, reachable
   via a different route (any real startup failure, not just opening
   Settings): "Organization UUID"/"User UUID" fields saved straight to
   `SharedPreferences` with zero connection to login. Removed. Its
   "Reset to defaults" recovery action is replaced with "Sign out and
   retry": clears `FfiSessionStorage`'s tokens and the persisted
   `organization_id`/`user_id`, unsets `hasRealFfiSessionKey`, and
   retries — which (per `restartApp`'s own gate, added in the previous
   piece) now correctly routes back to a real `FfiLoginScreen` rather
   than either crashing (the old "reset to defaults" reset
   `organization_id`/`user_id` to nothing while leaving a since-removed
   placeholder-default fallback that no longer exists) or reopening
   mobile-core under a stale identity. The Cloud Relay endpoint field is
   left editable here, since a bad relay URL is a real, distinct,
   non-identity cause of a startup failure this screen still needs to
   help recover from.

**What was deliberately left alone, and why:** neither fix touches how
a real login actually resolves identity (`ffi_login_screen.dart`,
unchanged) — this piece only removes the two free-text bypass routes
around it. The Cloud Relay endpoint remains editable in both screens;
it is a connection setting, not an identity claim, and editing it to a
wrong value cannot let anyone act as someone else.

**Verification actually run:**
- `git status` confirms only `mobile/lib/ui/app.dart`,
  `mobile/lib/ui/screens/settings.dart`, and
  `mobile/lib/ui/startup_error_screen.dart` changed — no Rust crate
  touched, so no `cargo test` applied here; `cargo check --workspace
  --exclude desktop-shell --exclude admin-shell` re-run anyway as a
  sanity check, clean (unsurprising).
- Confirmed via `grep` that `saveSettings` has no other call site
  anywhere in `mobile/lib` or `mobile/test` before renaming it to
  `saveRelayEndpoint`, so nothing else broke by removing the
  `organization`/`user` parameters.
- Confirmed via `grep` that no existing test file references
  `saveSettings`/`saveRelayEndpoint`, `organization`/`user`
  `TextEditingController`s in either changed screen, or otherwise
  depends on the removed fields — nothing in the existing test suite's
  expectations was contradicted.
- All three changed files hand-verified against this project's existing
  patterns and brace-balance-checked, but **not** compiled or run — no
  Dart/Flutter SDK exists in this sandbox, the same disclosed constraint
  as every other Dart change this session. Must be run through
  `flutter analyze` and exercised on a real device before being trusted
  in production, same as the login work it closes the gap on.

**Status:** the security hole described in the previous entry's third
disclosed gap is closed. Fix #2 (no `/api/auth/refresh` route,
lower severity — a session that silently degrades after an hour rather
than a way to gain unauthorized access) is the next piece of work, not
done in this entry.

## Fixed: added `POST /api/auth/refresh`, closing the token-refresh gap (Fix #2)

Per the project owner's own severity ranking — a real bug, not a
security hole, since nobody gains unauthorized access from a missing
refresh route; sessions just degrade (denied/logged-out) when they
shouldn't — this was done after, not before, the manual-UUID-entry fix.

**What was built:**

- `crates/bins/api-server/src/routes/auth.rs`: new `POST
  /api/auth/refresh` handler, `refresh()`. Takes `{refresh_token}`,
  validates it via the existing `validate_token(state, token,
  "refresh")` (the same function `authenticate_headers`/`logout`
  already use, just with `expected_type = "refresh"` instead of
  `"access"` — no new token-validation logic was written), re-fetches
  the user from the store and confirms `is_active` (mirrors `login`'s
  own "never trust cached claims from a token that could predate a
  demotion/deactivation" reasoning), then issues a **new** access token
  *and* a new refresh token via the existing `issue_token` helper.
  **Rotates**: the presented refresh token is added to
  `state.revoked_tokens` (the same in-memory set `logout` already
  writes to) so it can never be redeemed a second time — a caller must
  persist the new refresh token from every response, not just the new
  access token. Registered at `/api/auth/refresh` in `routes/mod.rs`,
  right alongside `/api/auth/login`.
- `mobile/lib/net/auth.dart`: `OnyxHttpAuthApi` gains `refresh({required
  refreshToken})`, calling the new route and updating `_client.auth` in
  place, returning the rotated refresh token for the caller to persist.
- `mobile/lib/main.dart`'s `_refreshHierarchyBestEffort` (added in the
  previous piece) now tries the stored access token first, and on
  failure redeems the stored refresh token via `OnyxHttpAuthApi.refresh`,
  persists the rotated tokens via `FfiSessionStorage`, and retries the
  hierarchy fetch once — closing the ~1-hour ceiling that piece's own
  entry disclosed. Still fully best-effort and non-blocking on startup,
  same as before; a refresh token that has itself expired (7 days) or
  been revoked still requires a real password login, which is
  unavoidable and not part of what this fix claims to solve.
- `mobile/lib/net/session_storage.dart`'s doc comment updated to
  reflect the route now exists and is used, rather than continuing to
  describe it as an open gap.

**Verification actually run:**
- `cargo check -p api-server` and `cargo clippy -p api-server
  --all-targets --no-deps -- -D warnings` — clean. Re-ran across the
  whole non-GTK workspace (`cargo check`/`clippy --workspace --exclude
  desktop-shell --exclude admin-shell`) too — also clean.
- New real, end-to-end HTTP tests,
  `crates/bins/api-server/tests/auth_refresh.rs`:
  `refresh_token_yields_a_working_new_access_token_and_rotates` (logs in
  as the seeded admin, redeems the refresh token, confirms the new
  access token actually works against a real authenticated endpoint —
  `GET /api/users/hierarchy` — not just that the response looked
  well-formed, confirms the *old* refresh token is rejected with 401 on
  reuse, and confirms the *new* refresh token still works) and
  `refresh_rejects_a_bogus_token_and_an_access_token_used_as_a_refresh_token`
  (a garbage string, and — importantly — a real *access* token
  presented where a refresh token belongs, are both rejected with 401,
  proving `expected_type` is actually enforced and not just accepted
  for any well-signed token). Both pass:
  `cargo test -p api-server --test auth_refresh` → 2 passed, 0 failed.
- Re-ran `mobile_access_gate` (2 passed) and the full `api-server` test
  suite alongside this change: the only failure is the same
  pre-existing, already-disclosed `query_id_normalization` bootstrap
  conflict (confirmed unrelated to this change in an earlier entry via
  `git stash` against unmodified `main`) — no new failures introduced.
- Dart changes (`auth.dart`'s `refresh()`, `main.dart`'s updated
  `_refreshHierarchyBestEffort`, `session_storage.dart`'s doc comment)
  hand-verified against this file's own existing patterns and
  brace-balance-checked, but **not** compiled or run — no Dart/Flutter
  SDK exists in this sandbox, the same disclosed constraint as every
  other Dart change this session.

**Status:** both disclosed gaps from the FFI-mode-login piece are now
closed — the security hole (Fix #1) and the token-refresh limitation
(Fix #2). The remaining, deliberately-untouched item from that same
piece is unchanged: `settings.dart`'s Cloud Relay endpoint field and
Sign-out action are the only identity-adjacent controls left, and
identity itself now only ever changes via a real login or sign-out.

## Fix #2, completed to the stricter standard actually asked for: proactive refresh + a real-expiry test

A follow-up task asked for both fixes again with more specific
acceptance criteria than the previous two entries met. Re-checked the
real, current code against every assumption in that task rather than
re-doing work already done — most of it, it turned out, already held:

- **Fix #1 (manual UUID entry) needed no further changes.** Re-read
  `settings.dart`/`startup_error_screen.dart`/`app.dart` directly:
  `saveSettings` no longer exists (renamed to `saveRelayEndpoint`,
  relay-only), both screens show identity read-only, and
  `organization_id`/`user_id` in `SharedPreferences` are only ever
  written by `ffi_login_screen.dart`'s real login response or
  `http_login_screen.dart`'s login flow — confirmed via `grep` across
  every `setString('organization_id'/'user_id', ...)` call site.
  Nothing further to fix.
- **One adjacent thing checked and found genuinely different, not
  silently equated with the fixed bug:** `http_login_screen.dart` (the
  *HTTP*-transport login screen, unrelated to the FFI-mode screens the
  security hole was in) still has a free-text "Organization UUID"
  field. Read `routes/command.rs`/`routes/query.rs` directly to check
  whether this is exploitable the same way: it is not — both
  `/api/command` and `/api/query` independently reject
  `envelope.organization_id != auth.organization_id` with
  `403 TENANT_MISMATCH`/`FORBIDDEN`, so a wrong or malicious org id
  typed there only ever produces failed requests against a real
  server, never impersonation, since the HTTP transport has no local
  `AppState` trusting client-asserted identity the way FFI mode's local
  command dispatch did. Left untouched — flagged as worth a second
  look someday, not treated as part of this fix.
- **Fix #2 (`/api/auth/refresh`) needed two real additions the earlier
  entry hadn't done:**
  1. **Proactive renewal, not just reactive-after-failure.** The
     earlier implementation only ever called `refresh` once, at
     startup, and only after a hierarchy fetch had already failed. A
     session left open for hours had no mechanism to renew its token
     again after that single startup attempt. `OnyxController`
     (`ui/app.dart`) now runs a `Timer.periodic` (45 minutes — safely
     inside the confirmed 3600-second/1-hour access-token TTL read
     directly from `issue_token(&state, &user, "access", 3600)`'s two
     call sites in `auth.rs`) for the lifetime of the running app, only
     when `api is OnyxMobile` (the HTTP transport has no local cache to
     keep fresh and re-authenticates every restart by design already).
     The reactive retry-on-401 path in `refreshHierarchyBestEffort`
     (now public, called by both the one-shot startup path and this
     timer) stays as a safety net, not the primary mechanism.
  2. **A real test that reaches actual expiry, not just proves the
     endpoint returns 200 in isolation.** Added
     `access_token_that_has_actually_expired_is_rejected_and_refresh_recovers`
     to `auth_refresh.rs`: fixes `ONYX_AUTHORITY_SIGNING_KEY` to the
     same non-production default `ApiState::new` itself falls back to,
     decodes a real, freshly-issued access token's real claims with the
     same `Ed25519JwtCodec` the server uses, rewrites only
     `exp`/`iat` to genuinely-in-the-past values, and re-signs with the
     same key — a real, validly-signed, genuinely-expired token, not a
     mock or a malformed one. Confirms that token is rejected
     (`401`) by the exact `GET /api/users/hierarchy` endpoint mobile's
     `fetchHierarchyJson` calls to populate the approval-authority
     cache, then confirms the refresh token issued alongside it
     redeems a replacement that immediately works again against that
     same endpoint. This is the server-side half of "the
     approval-authority cache keeps working across a refresh" — the
     Dart/FFI half (an actual `mobile_core_set_hierarchy` call
     succeeding post-refresh) cannot be exercised in this sandbox at
     all, disclosed below, not glossed over.
- **Checked, per the task's explicit instruction, whether
  `desktop-shell` has ever used a refresh path: it has not.**
  `crates/bins/desktop-shell/src/lib.rs` calls `hierarchy_cache.refresh(...)`
  in two places — confirmed by reading both call sites directly this
  is `HierarchyCache::refresh` (an unrelated method that re-fetches the
  reporting-line tree), not a token refresh. `desktop-shell` has never
  redeemed a refresh token and still doesn't; its access token expires
  after the same 1 hour with no renewal, identical to mobile's
  pre-Fix-#2 state. Not fixed here — out of this task's stated scope
  (mobile FFI session only) — and named explicitly rather than left for
  someone to discover independently later.

**Verification actually run:**
- `cargo test -p api-server --test auth_refresh` → **3 passed, 0
  failed** (the two from the prior entry plus the new real-expiry
  test). `cargo clippy -p api-server --all-targets --no-deps -- -D
  warnings` and `cargo check`/`clippy --workspace --exclude
  desktop-shell --exclude admin-shell` — all clean.
- Full `cargo test -p api-server` re-run: only the same pre-existing,
  already-disclosed `query_id_normalization` bootstrap-conflict failure
  — no new failures from this change.
- `mobile/lib/ui/app.dart`'s new `Timer.periodic` and
  `mobile/lib/main.dart`'s renamed public `refreshHierarchyBestEffort`
  hand-verified against this project's existing patterns and
  brace-balance-checked, but **not** compiled or run — no Dart/Flutter
  SDK exists in this sandbox, the same disclosed constraint as every
  other Dart change this session. The claim that the approval-authority
  cache "keeps working across a refresh" is proven server-side only
  (the new Rust test above); the actual Dart timer firing and calling
  `mobile_core_set_hierarchy` successfully has not been observed
  running, only written to match tested, working pieces
  (`refreshHierarchyBestEffort` itself, `OnyxHttpAuthApi.refresh`) that
  were each independently verified where they could be.

**Status:** both fixes now meet the stricter standard asked for. Fix
#1 required no new work; Fix #2 gained proactive renewal and a real
deterministic-expiry test. The `http_login_screen.dart` free-text org-id
field and `desktop-shell`'s own missing token refresh are both real,
adjacent, and explicitly not fixed here — named rather than left for
a future correction to have to find independently.

## Fixed three real, independently-verified CI failures — and, for the first time this session, a real local Flutter/Android toolchain

A follow-up task supplied three confirmed-real failures from an
independent compile/test pass (Manus, GitHub Actions run `33127510920`
against commit `769bdcb`). Unlike every prior mobile-verification claim
this session, this one was checked with a **real, newly-installed local
Flutter 3.47.2 + Android SDK toolchain** — not disclosed-as-unverifiable,
for once. How that happened, and what it changed about this session's
own prior disclosures, matters enough to record in full.

**Why a real toolchain became possible now, when it wasn't all session.**
Every prior entry this session correctly disclosed "no Dart/Flutter SDK
exists in this sandbox." That was true of the sandbox as first found —
but nobody had ever actually tried installing one until this task
required literal `flutter build apk` output, not a summary. Checked
disk first (`df -h /`): 3.8 GB free, nowhere near enough for a ~1.6 GB
Flutter SDK download plus an Android SDK/Gradle toolchain. The
`target/` directory (Rust build cache, fully regenerable via `cargo
build`) was consuming 24 GB of that; `cargo clean` freed it back to 28
GB available, at which point installing a real Flutter 3.47.2 (the
exact version the verification pass itself used, confirmed by checking
Google's own `releases_linux.json` — the current stable release
happened to already be `3.47.2`) and a real Android SDK/Gradle/JDK
toolchain both fit comfortably. Real network access (`storage.googleapis.com`,
`dl.google.com` via this sandbox's proxy) and `apt-get`/`sudo`
both turned out to work too — also never previously tried. None of this
was available in earlier pieces because the earlier tasks never
required it and never asked me to try; it is disclosed here specifically
so a future entry doesn't re-assert "impossible" without re-checking
first.

**The same real toolchain also unblocked `desktop-shell`/`admin-shell`
compilation for the first time this session** (`libgtk-3-dev
libwebkit2gtk-4.1-dev libsoup-3.0-dev libjavascriptcoregtk-4.1-dev
libayatana-appindicator3-dev librsvg2-dev` — the exact package list
`ci.yml`'s own `check` job already installs), which is what actually
let Fix #2 below be verified as a real `cargo clippy`/`cargo check`/
`cargo test` pass rather than the same disclosed GTK gap repeated once
more.

### Fix #1 — `mobile/lib/net/auth.dart:50` Dart parser ambiguity

Reproduced for real first: `flutter analyze` on a fresh `flutter pub
get` gave the identical `missing_identifier`/`expected_token` pair at
`lib/net/auth.dart:50:107` the task described. Applied the suggested
fix — split the ternary + null-aware-index + cast chain into three
typed locals:
```dart
final responseData = e.response?.data;
final error = responseData is Map ? responseData['error'] : null;
final code = error is Map ? error['code'] as String? : null;
```
No behavior change: same three-step null-safe walk (`response.data` →
`['error']` → `['code']`), same fallback to `null`, same
`MOBILE_ACCESS_RESTRICTED` comparison immediately after.

**Confirmed no other file has the same pattern**: `grep -rn '?\[' mobile/lib
--include=*.dart` found exactly one other null-aware-index use in the
entire mobile codebase (`onyx_http_api.dart:70`,
`_client.auth.user?['id'] as String? ?? ''`) — a `??`
null-coalescing expression, not a `? :` ternary, so it does not hit the
same parser ambiguity (confirmed by it never having appeared in any
`flutter analyze` failure, including the one just reproduced).

**Verification, real command output:**
```
$ flutter analyze
Analyzing mobile...
No issues found! (ran in 7.7s)

$ flutter test
...
00:03 +8 ~1: All tests passed!
```
8 passed, 1 skipped (the same `p2p_sync_test.dart` device-only skip the
verification pass reported) — identical to the cited baseline. This is
also the first real compile+test confirmation, this whole session, that
every Dart change made across every earlier mobile piece (approval
authority, class-based access, file sharing, real FFI login, the
security-hole fix, the token-refresh wiring) actually builds and passes
— all of it had only ever been hand-verified against existing patterns
before now.

### Fix #2 — `desktop-shell::login` exceeds Clippy's `too_many_arguments`

Reproduced for real first (the first time `cargo clippy --workspace`
could even reach `desktop-shell` this session):
```
error: this function has too many arguments (8/7)
   --> crates/bins/desktop-shell/src/lib.rs:368:1
```
**Checked every other Tauri command in this file, and confirmed
`admin-shell` has none at all**, before touching anything (a
Python pass over every `#[tauri::command]` function's real parameter
count): `execute_command` (2), `execute_query` (3), `subscribe_events`
(3), `get_sync_status` (1), `upload_file` (5), `download_file` (3),
`store_secret` (3), `get_secret` (2), `delete_secret` (2),
`get_current_session` (2), `login` (8), `logout` (4).  Only `login`
exceeds 7; nothing else is close enough to be worth touching
speculatively. `grep -rl tauri::command crates/bins/admin-shell` found
zero matches — confirmed directly, not assumed, that `admin-shell` has
no Tauri commands at all (it is a thin HTTP client; its own `Login.tsx`
calls `apiClient.post` against `api-server`, not a native command), so
there is no "admin-shell equivalent" to check for the same shape.

Fixed by grouping the three real input values into one struct:
```rust
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    server_address: String,
    username: String,
    password: String,
}
```
`login` now takes `credentials: LoginRequest` in place of the three
separate `String` parameters — 6 parameters total (5 injected Tauri
state handles + 1 struct), under the limit. `#[serde(rename_all =
"camelCase")]` so the existing camelCase JS-side `invoke` convention
(every other command in this file already expects camelCase keys)
needs no per-field rename. Updated the one real call site,
`desktop-shell/ui/src/pages/Login.tsx`, to nest the three fields under
a `credentials` key to match. Confirmed via `grep` this is the only
call site.

**Verification, real command output:**
```
$ cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.48s
$ cargo check --workspace
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.87s
$ cargo test -p desktop-shell
test result: FAILED. 9 passed; 2 failed; 1 ignored; 0 measured; 0 filtered out
```
Both `clippy`/`check` are clean across the **entire workspace**,
including `desktop-shell`/`admin-shell`, for the first time this
session. The two `secure_storage::keyring_adapter` test failures are
real but pre-existing and unrelated to this change — confirmed via
`git stash` on unmodified `main` (identical two failures) — and are
exactly the gap `ci.yml`'s own `check` job comment already documents
and works around: a bare Linux environment has no D-Bus session bus or
Secret Service daemon, so the real (deliberately-not-mocked) `keyring`
crate tests fail with "No default store has been set"; CI's own fix is
to start a real `dbus-launch`/`gnome-keyring-daemon` session first,
which this sandbox does not have configured. Not fixed here (out of
scope, and CI already has the real fix), but disclosed as a newly-found
(only reachable now that `desktop-shell` compiles at all here) local
environment gap rather than silently treated as "fixed."

### Fix #3 — Android Gradle Plugin below Flutter's minimum

Reproduced for real first, against a real, newly-installed Android SDK
(`platforms;android-35`, `platforms;android-36`, `build-tools;28.0.3`
and `;35.0.0` — the exact versions this real Flutter 3.47.2 itself
demanded via `flutter doctor -v`, not guessed):
```
Error: Your project's Android Gradle Plugin version (Android Gradle Plugin version 8.6.0) is lower than Flutter's minimum supported version of Android Gradle Plugin version 8.11.1.
```
**8.11.1 confirmed as this environment's real minimum** — not copied
blindly from the task text, independently reproduced against a real
Flutter 3.47.2 install (the same version number the task's own
verification pass used, and, as it happens, the exact current stable
release as of this session). Bumped
`mobile/android/settings.gradle`'s `com.android.application` plugin
version from `8.6.0` to `8.11.1`.

**This did cascade, exactly as the task anticipated — checked for real,
not assumed:** the AGP bump alone produced a second, real error:
```
Error: Your project's Kotlin version (2.0.20) is lower than Flutter's minimum supported version of 2.2.20.
```
Bumped `org.jetbrains.kotlin.android` from `2.0.20` to `2.2.20` in the
same file. The Gradle wrapper (`gradle-wrapper.properties`, already
pinned to `9.1.0`) needed **no** change — confirmed by the build
proceeding past both version checks and completing successfully
without touching it.

**Verification, real command output** (not just past the version-check
error — real, complete APK assembly):
```
$ flutter build apk --debug
Running Gradle task 'assembleDebug'...                            195.5s
✓ Built build/app/outputs/flutter-apk/app-debug.apk
EXIT:0

$ ls -lh build/app/outputs/flutter-apk/
-rw-r--r-- 1 root root 163M Aug 28 05:07 app-debug.apk
```
A real, 163 MB debug APK now exists on disk. One honest limitation of
this local reproduction, disclosed rather than glossed over: this
build did **not** run `mobile/tool/build_rust_android.sh` (the script
that cross-compiles `mobile-core` for `arm64-v8a`/`armeabi-v7a`/`x86_64`
via `cargo-ndk` into `android/app/src/main/jniLibs`) — confirmed no
`.so` files exist anywhere under `mobile/android`. The Gradle/Dart
assembly step this task's checklist asked to verify succeeds
regardless of whether the native library is present (packaging doesn't
require it; only calling into `mobile-core` at runtime would), so this
faithfully proves the AGP/Kotlin version fix itself, but does not prove
the full CI `mobile-android` job's native cross-compile step still
works — that step was not exercised locally (installing an NDK
toolchain and cross-compiling `mobile-core` was judged out of scope for
verifying a Gradle *version* fix) and is left to the real CI dispatch
below to confirm end to end.

Also confirmed `mobile/tool/ensure_platform_scaffold.sh` — which `ci.yml`'s
`mobile-android` job runs immediately before this — is a no-op given
this repo's current state (`android/gradlew` and
`ios/Runner.xcodeproj/project.pbxproj` both already exist, so its early
`exit 0` guard fires), meaning the local build above used the exact
same `android/` directory the real CI job would.

### The `cargo fmt` judgment call, made explicitly rather than silently either way

`cargo fmt --all -- --check` was run as the task invited. Real result:
**58 diff locations across ~15 files**, spanning code from this
session's earlier pieces (`hierarchy_cache.rs`, `auth_refresh.rs`,
`mobile_access_gate.rs`, `session.rs`, `user_store.rs`, etc.) and code
that predates this session entirely. **Decision: not included.** This
is not the "trivial, low-risk" case the task flagged as worth a call —
it is a 58-location, ~15-file reformatting sweep touching code far
outside these three fixes, which would itself be exactly the kind of
scope expansion the task explicitly said not to do. One exception was
made: the single new line this fix's own new code introduced (`let
LoginRequest { server_address, username, password } = credentials;`)
was reformatted to match this repo's real rustfmt output before
committing, since leaving freshly-written code non-compliant with the
formatter the codebase already uses would be careless, not scope
discipline. Every other diff — including two in `desktop-shell/src/lib.rs`
and `session.rs` from a prior session entry — was left untouched.

### One more real, disclosed thing found while doing this: `pubspec.lock` and `analysis_options.yaml`

Running `flutter pub get` for the first time against this exact
Flutter/Dart version generated `mobile/pubspec.lock` (new, untracked,
not gitignored either) and silently rewrote
`mobile/analysis_options.yaml` (`"Upgrading analysis_options.yaml to
exclude build and platform directories."` — a real message the Flutter
tool itself printed, not something manually edited). **Decision:**
`pubspec.lock` was left untracked — this repo has never committed one
(no prior commit history has it, and it isn't gitignored either, which
reads as "nobody has run `flutter pub get` and committed the result
before," not as an intentional exclusion), and committing it now would
be a real, unrequested convention change outside this task's three
items. `analysis_options.yaml`'s automatic rewrite **was** committed:
it is a toolchain-driven migration tied to using this exact Flutter
version (not a manual style choice), `flutter analyze` was already
confirmed clean with it in place, and leaving it uncommitted would mean
every future real `flutter analyze` run (including in CI, once it
upgrades past whatever `flutter-action@v2`'s `channel: stable` resolves
to) silently re-applies the identical migration on its own, which is a
worse outcome for reproducibility than committing the tool's own output
once, here, disclosed.

### Verification checklist, exactly as the task asked for it

| Check | Result |
|---|---|
| `flutter analyze` | 0 issues |
| `flutter test` | 8 passed, 1 skipped (matches baseline) |
| `flutter build apk --debug` | real, successful assembly (163 MB `app-debug.apk`) |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo check --workspace` | clean |
| `cargo fmt --all -- --check` | 58 pre-existing diffs found, deliberately not included (see above) |
| Fresh `workflow_dispatch` of `ci.yml` | see below |

### The real `workflow_dispatch` result (run `33144081403`, commit `68e4cf2`)

| Job | Baseline (`33127510920`, `769bdcb`) | This run (`68e4cf2`) |
|---|---|---|
| `check` | failure (Format step; Clippy/Build/Test never reached) | failure — **same** Format step, same pre-existing reason, Clippy/Build/Test again never reached |
| `web` | success | success |
| `deploy-check` | success | success |
| `mobile-dart` | **failure** (`Analyze` step — the exact `auth.dart:50` bug) | **success** — `Analyze`, `flutter test`, and the mobile structural contract all pass |
| `native-ui-evidence` | success | success |
| `mobile-android` | **skipped** (blocked on `mobile-dart`) | **success** — including the real `cargo-ndk` cross-compile (`build_rust_android.sh`, ~10 min) and `flutter build apk --debug`, artifact uploaded |
| `mobile-ios` | **skipped** (blocked on `mobile-dart`) | **failure** — a genuinely different, pre-existing Swift compile error, `Cannot find 'BackgroundService' in scope` (`mobile/ios/Runner/AppDelegate.swift:10`), unrelated to any of the three fixes in this task |
| `load-smoke` | skipped | skipped (same trigger condition) |

**`check` still fails, exactly as anticipated by the `cargo fmt` decision
above** — the job's `Format` step runs before `Clippy`, so the
pre-existing, disclosed, out-of-scope formatting drift keeps `Clippy`
(Fix #2's real CI proof) from ever running in this job. Fix #2 was
verified for real, but locally (`cargo clippy --workspace --all-targets
-- -D warnings`, clean) rather than by this specific job going green —
a direct, foreseeable, and already-disclosed consequence of not running
`cargo fmt --all`, not a surprise.

**`mobile-dart` and `mobile-android` — the two jobs this task actually
named — both pass for real,** the first time either has ever reached a
non-skipped state in this repository's CI history for `mobile-android`
specifically (it depends on `mobile-dart`, which has never passed
before this fix).

**`mobile-ios` is a real, newly-*visible* (not newly-*caused*) failure.**
It was `skipped` in the baseline run and every prior run for the same
reason `mobile-android` was — blocked on `mobile-dart`, which always
failed first. This is the first time `mobile-ios` has ever actually
executed in this repository's CI. Its failure is a Swift compile error
in a file (`AppDelegate.swift`) none of this task's three fixes
touched, on a platform (`macos-latest`, Xcode/CocoaPods) none of them
concern. Not fixed here: out of this task's explicit scope, and
newly-discovered only because fixing `mobile-dart` was what finally let
CI reach it at all. Flagged for a future task rather than silently
absorbed into "done" or silently left for someone else to rediscover
from scratch.

## Clearing the `cargo fmt` drift — getting `check`'s Format step green

### What changed

A fresh `git pull` on `main` showed the tip had moved to `25d6172` (a
merge of `fix/ci-platform-validation`, forked from `bf726ae`) since the
last task — not `8878fa1` as assumed going in. That merge had, as a side
effect of its own unrelated work, already applied `rustfmt` to nearly
all of the ~58 diffs across ~15 files previously logged as pre-existing,
out-of-scope drift. Re-running `cargo fmt --all -- --check` against the
real, current tip found exactly **one** diff left: the same
`crates/bins/desktop-shell/src/lib.rs` block (from `0400f81`, the mobile
approval-authority piece) already identified by name in the prior
task's report. Running `cargo fmt --all` for real reformatted that one
`Arc::new(...) as Arc<dyn OwnerAuthority>` expression's line-wrapping —
purely whitespace/layout, no tokens added, removed, or reordered beyond
where rustfmt breaks a long chained call across lines. `git diff --stat`
confirms the scope: 1 file, 5 insertions, 2 deletions.

### Why this was judged safe to commit on its own

`cargo fmt` restricts itself to whitespace and line layout by
construction, but per this project's own standing discipline that gets
verified rather than assumed:

- `cargo fmt --all -- --check` — clean (0 diffs) after the real run.
- `cargo check --workspace` — clean, matching the pre-formatting state.
- `cargo clippy --workspace --all-targets -- -D warnings` — clean,
  matching the known-clean state from `68e4cf2`.
- `cargo test --workspace` — see below; not a clean apples-to-apples
  local comparison, but nothing in the result points at the formatting
  diff as a cause.

### The `cargo test --workspace` complication (disclosed, not glossed over)

This sandbox has much less free disk than a full workspace test build
needs — a clean `cargo test --workspace` build alone consumes on the
order of 15–16 GiB of `target/`, and doing it back-to-back with `check`
and `clippy` (each leaving their own `dev`-profile artifacts behind)
exhausted the disk mid-build the first two attempts, aborting with
`rustc-LLVM ERROR: IO failure on output stream: No space left on
device` — an infrastructure failure, not a real test result (`grep` for
`test result:`/`FAILED` against those logs returned nothing; no test
binary ever finished linking). Freed by `cargo clean` between each of
`check`, `clippy`, and `test` (tooling from the prior mobile task —
`/opt/android-sdk`, `/opt/flutter`, `/root/.gradle` — was left in place
rather than removed, since deleting it wasn't authorized for this task;
`cargo clean` on the Rust `target/` alone was sufficient and safe to
redo, since it's pure derived output).

With disk no longer the blocker, a full `cargo test --workspace
--no-fail-fast` run completed. It did **not** reproduce the same 6
named baseline failures this task expected to re-confirm
(`query_id_normalization`, `relay_switchboard`,
`staff_loan_authorization`, `user_hierarchy_admin_routes`,
`staff_profile_routes`, `team_leader_precheck_authorization`) — every
one of those 6 **passed** in this run. Instead, a different set failed:
2 `desktop-shell::secure_storage::keyring_adapter` tests, and roughly
16 tests across `persistence-postgres` (concurrency/idempotency/outbox/
repository) and `client-composition`'s SQLite/journey integration
tests. Investigated rather than assumed unrelated:

- The `keyring_adapter` failures are a documented property of the
  `keyring` crate's Linux Secret Service backend, confirmed against its
  own docs (via Context7): it depends on a D-Bus session daemon
  (gnome-keyring/KeePassXC) and is explicitly documented as unsuitable
  for headless environments. This sandbox has no D-Bus session bus.
- The `persistence-postgres` and journey/integration failures are
  consistent with the same root cause class — this sandbox has no live
  Postgres instance for those tests to run against, unlike CI's `check`
  job which provisions one as a service container.

Neither category is plausibly caused by a whitespace-only edit to
`desktop-shell/src/lib.rs`'s `owner_authority` closure — nothing in that
diff touches keyring code, Postgres access, or test fixtures. This
sandbox simply cannot reproduce the CI job's exact test-pass baseline
locally (it's missing services CI provisions), so CI's own `check` run
— which does have Postgres and runs in a normal Ubuntu runner, not this
constrained container — remains the authoritative comparison for the 6
named failures, and its result is reported alongside this entry rather
than asserted from the local run.

### Verification checklist

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | clean (0 diffs), after real `cargo fmt --all` |
| `cargo check --workspace` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace --no-fail-fast` (local, no Postgres/D-Bus) | 6 previously-named baseline failures now pass; distinct, disk/infra-explained failures in `keyring_adapter` and `persistence-postgres`/journey tests, unrelated to this diff |
| Fresh `workflow_dispatch` of `ci.yml` | `check` green for the first time (run `33176531805`, commit `f4805a3`) — see below |

This is a pure-formatting commit: `crates/bins/desktop-shell/src/lib.rs`
only, nothing else staged alongside it.

### The real `workflow_dispatch` result (run `33176531805`, commit `f4805a3`)

| Job | Baseline (`33144081403`, `68e4cf2`) | This run (`f4805a3`) |
|---|---|---|
| `check` | failure (Format step) | **success** — Format, Clippy, Build, Test, Docs, migration idempotency, and contract verification all pass, first time this job has ever gone fully green |
| `web` | success | success |
| `deploy-check` | success | success |
| `mobile-dart` | success | success |
| `native-ui-evidence` | success | success |
| `mobile-android` | success | success |
| `mobile-ios` | failure (`AppDelegate.swift:10`, `BackgroundService` not in scope) | failure — confirmed **same** error, same file, same line, unrelated to this fix |
| `load-smoke` | skipped (gated on `check`) | **failure** — first time this job has ever executed |

**`check` finally goes green end-to-end**, confirming the local
`cargo test --workspace` divergence noted above really was this
sandbox's missing D-Bus/Postgres, not a real regression: CI's own
`check` job starts a D-Bus session with an unlocked gnome-keyring
specifically for the `secure_storage` tests, and runs Postgres as a
service container, and with both present, `Test` passes clean —
including the `keyring_adapter` and `persistence-postgres` tests that
failed locally.

**`load-smoke` is a newly-*visible* (not newly-*caused*) failure**,
the same category as `mobile-ios` in the prior entry. It never ran
before because it depends on `check`, which never passed. Its actual
failure: the job's own "Start optimized API" step polls
`curl http://127.0.0.1:3000` for 60s and the server never comes up in
that window, so `k6`'s `setup()` fails immediately with "connection
refused" before a single real load-test request is attempted. Nothing
in this task's one-line formatting diff touches that job, that script,
or API startup. Not fixed here: out of scope, flagged for a future
task rather than silently absorbed into "done."

## Fixing `load-smoke`'s server-startup timeout

### Diagnosis, not assumption

The task handed to this session came with a plausible-sounding
hypothesis already attached — that `cargo run --package api-server`
was a *debug* build compiling cold on an uncached runner within the
60s health-check window. Read against the actual workflow file before
touching anything, that hypothesis was already half wrong: the "Start
optimized API" step (`.github/workflows/ci.yml`) was already running
`cargo run --release --package api-server`, not a debug build. Assuming
the stated hypothesis and "fixing" the wrong half of it (e.g. adding
`--release` to something already `--release`) would have shipped a
no-op.

What actually confirmed the real cause was in the failed run's own log
(run `33176531805`, job `98871920069`, retrieved via
`mcp__github__get_job_logs`), at the very end during the runner's
orphan-process cleanup, well after `k6` had already failed and the
job was tearing down:

```
Terminate orphan process: pid (7051) (rustc)
Terminate orphan process: pid (7183) (rustc)
```

`rustc` was still running when the 60-second health-check loop gave up
and the whole job moved on. That is direct, load-bearing evidence —
not an inference from timing alone — that the server process never
finished *compiling*, let alone binding port 3000: this genuinely is
case 1 from the task's own diagnostic script ("does it crash" vs.
"does it just take longer than 60s"), and the second op, not a crash,
config error, or missing env var. `api.log` itself (the redirected
server stdout/stderr) was correspondingly empty/never-written in the
run's artifacts — consistent with the process never getting past
`rustc` to produce a running binary at all.

With the crash/config-error branch ruled out, the task's own second
diagnostic question — caching — was checked directly against the
workflow file rather than assumed: `check` (line ~55) already has a
`Swatinem/rust-cache@v2` step; `load-smoke` had none at all. Every
other Rust-compiling job in this workflow either shares `check`'s
cache indirectly (by not compiling anything itself) or has its own
cache step; `load-smoke` was the one job doing a from-scratch release
compile of the entire dependency tree (tokio, sqlx, axum, and
everything api-server pulls in) with nothing cached, on every single
run.

### The fix: both preferred options at once, not either/or

The task's ranked list treated "add caching" and "separate build from
run" as alternatives to try in order. They aren't mutually exclusive
here, and doing only one leaves a real gap:

1. **Caching alone**, without splitting the build out of `cargo run`,
   would still count whatever compile time remains (now much shorter,
   but non-zero — `cache-workspace-crates` defaults to `false` in
   Swatinem/rust-cache, confirmed via Context7 against the action's
   own docs, so only third-party dependency artifacts are cached, not
   `api-server`'s own crate output) against the same 60s window used
   to detect server *startup* problems. A slow runner or a slightly
   larger dependency delta could still blow through it, and a real
   startup regression would look identical to a compile hiccup in the
   logs — exactly the ambiguity this task was asked to resolve.

2. **Splitting build from run alone**, without caching, would still
   pay the full cold compile of the entire dependency tree every run
   — just no longer racing a 60s clock while doing it, so the failure
   mode changes from "times out" to "takes several minutes longer than
   it should," which is real waste on every single `load-smoke` run
   forever, not a one-time cost.

Applied both, in `.github/workflows/ci.yml`:

- **`check`'s existing `Swatinem/rust-cache@v2` step now takes
  `shared-key: "release-build"`.** `load-smoke` declares
  `needs: check`, so it only starts once `check`'s own job — including
  its post-job cache save — has fully completed in the same workflow
  run. Confirmed against the action's own docs (via Context7) that
  `shared-key` is exactly the documented mechanism for exactly this:
  letting a later job in the same run reuse an earlier job's cache
  instead of each job getting an isolated one (the default,
  `add-job-id-key: true` behavior). `check` already runs
  `cargo build --workspace --release` (line ~66), which compiles the
  same dependency tree `load-smoke` needs; sharing the key means
  `load-smoke` restores those already-built dependency artifacts
  instead of recompiling axum, sqlx, tokio, etc. from zero.
- **`load-smoke` gets its own `Swatinem/rust-cache@v2` step**, same
  `shared-key: "release-build"`, added right after the toolchain setup.
- **The build is now its own step** (`Build API release binary`,
  running `cargo build --release --package api-server`), given the
  job's full remaining time budget rather than sharing the 60s loop.
- **"Start optimized API" now execs the already-built binary directly**
  (`./target/release/api-server`) instead of `cargo run --release`,
  which would otherwise re-invoke Cargo's own (now near-instant,
  post-build) up-to-date check on every start — harmless once already
  built, but removing it makes the step do exactly one thing: start
  the binary and wait for it to answer `/health`. The 60-iteration,
  1s-interval health-check loop is now actually measuring server
  startup time, not compilation, which is what it was always supposed
  to measure.

This composes with the existing `postgres` service container and env
vars in the job unchanged — nothing about database wiring, the smoke
test script, or the load profile (100 VUs / 60s) was touched, matching
the task's explicit scope.

### Verification

| Check | Result |
|---|---|
| `python3 -c "import yaml; yaml.safe_load(...)"` on `ci.yml` | valid YAML |
| `actionlint` | not available in this sandbox; not run |
| Fresh `workflow_dispatch` of `ci.yml` | see the real run's job table and `load-smoke` step timing below |

Not touched, per the task's explicit exclusions: `mobile-ios`'s
`BackgroundService` Swift error (still the same disclosed, unrelated,
pre-existing failure), and the 6 pre-existing `api-server` integration
test failures inside `check`'s own `Test` step (still out of scope).

### The real `workflow_dispatch` result (run `33196473346`, commit `e0d94a7`)

| Job | Baseline (`33176531805`, `f4805a3`) | This run (`e0d94a7`) |
|---|---|---|
| `check` | success | success |
| `web` | success | success |
| `deploy-check` | success | success |
| `mobile-dart` | success | success |
| `native-ui-evidence` | success | success |
| `mobile-android` | success | success |
| `mobile-ios` | failure (`AppDelegate.swift:10`, `BackgroundService` not in scope) | failure — confirmed **same** error, same file, same line, unrelated to this fix |
| `load-smoke` | **failure** (`rustc` still an orphan process at cleanup; server never bound the port in 60s) | **success** — for a real reason, detailed below |

**`load-smoke` passes for real, not just "the timeout logic changed."**
Its own step timings from this run:

- `Build API release binary` (new step): **started 18:11:12, finished
  18:13:12 — 120s.** This is the first-ever population of the new
  `release-build` shared cache key, so this number already includes
  whatever cold-compile cost remained; it is not a warm-cache number.
  Contrast with the baseline: at the old 60s mark, the equivalent work
  (`cargo run --release`) had *not finished compiling at all* —
  confirmed by the orphaned `rustc` processes still running at job
  cleanup. 120s-to-a-finished-binary vs. never-finishes-in-60s is the
  real before/after: the fix didn't make the build faster in the
  abstract, it gave the build a place to run to completion instead of
  racing a clock designed to measure something else.
- `Start optimized API` (the health-check loop, now just execing the
  already-built binary): **started 18:13:12, finished 18:13:13 — 1
  second.** This is the number the 60-iteration loop was always meant
  to measure — real server startup time — and now it's actually
  measuring it, cleanly separated from compilation.
- `k6 run ... --vus 100 --duration 60s`: **ran the full 60.2s**,
  completing **36,514 real iterations** against the running server at
  ~607 req/s, 42 MB received / 83 MB sent, job step conclusion
  `success`. Contrast with the baseline, where the entire k6 run
  produced exactly one HTTP attempt total (`connection refused`) before
  `setup()` threw and the run aborted in under a second. This run is a
  real load test that actually exercised the server, not a
  fast-failing stub.

**On the caching half of the fix specifically:** this run cannot yet
show a warm-cache speedup for `load-smoke`'s own restore, because
`release-build` is a brand-new cache key with nothing to restore from
before this run populated it (`check`'s own `Run Swatinem/rust-cache@v2`
step this run took only ~2s — a cache-miss/nothing-to-restore duration,
not a several-hundred-MB download; its `Build` step then ran a full
~8 minutes, consistent with a genuinely cold compile). The caching
benefit that matters for *this* task — `load-smoke` restoring the
dependency tree `check` already compiled in the *same* run, rather
than compiling it a second time from zero — is real and already
visible in the numbers above: `load-smoke`'s own `Build API release
binary` step only had to compile `api-server`'s own crate and its
immediate workspace-local dependencies on top of that shared,
already-built dependency tree (confirmed via Context7 against
Swatinem/rust-cache's own docs: `cache-workspace-crates` defaults to
`false`, so only third-party dependency artifacts carry over, not
workspace crates' own compiled output) — which is why it finished in
120s rather than repeating anything close to `check`'s own ~8-minute
full-workspace compile. A second `workflow_dispatch` run, now that the
`release-build` key holds a saved cache from this run, would show
`check`'s own restore step take noticeably longer than 2s (an actual
download) and its `Build` step drop well below 8 minutes; not run here
since `load-smoke` passing for a real, evidenced reason was the task's
actual bar, already cleared.

## Fixing `mobile-ios`'s "Cannot find 'BackgroundService' in scope" error

### A stale-checkout discrepancy, resolved before this task started

The task handed to this session opened by pointedly requiring a fresh,
*verified* `git fetch`/`log` rather than an assumed local state — a
direct response to a real discrepancy raised immediately prior: the
user's local copy of `ci.yml` showed `load-smoke`'s "Start optimized
API" step as plain `cargo run --package api-server` (no `--release`),
contradicting the previous report's claim that it already read `cargo
run --release --package api-server` before that fix. Traced with
`git log -p --all -- .github/workflows/ci.yml`: the line was
originally added without `--release` in commit `62fea331`
(2026-08-16), then `--release` was added in `bf726ae5` ("fix:
stabilize cross-platform CI validation", 2026-08-27 23:13 UTC), which
merged into `main` via `25d6172` (2026-08-28 12:46 UTC) — before either
of the two commits in the previous task (`f4805a3` at 13:40 UTC,
`e0d94a7` at 17:49 UTC). `git merge-base --is-ancestor bf726ae HEAD`
confirmed `bf726ae` really is an ancestor of current `main`. The
previous report's claim was accurate to the file as it existed on
`main` at the time of that work; the user's copy was simply stale
relative to `main`'s tip, not a divergent branch. Resolved by directly
walking the commit history rather than re-asserting either side's
memory of the file's contents. This task's own instruction to verify
`git fetch origin main && git log origin/main --oneline -5` before
doing anything else was followed for exactly this reason, and
confirmed tip at `9535efe` before any other action below was taken.

### Diagnosis, independently confirmed

The task arrived with a diagnosis already attached (`BackgroundService`
present on disk, referenced from `AppDelegate.swift`, but absent from
`Runner.xcodeproj/project.pbxproj`) and an explicit instruction not to
just trust it. Checked directly rather than assumed:

- `mobile/ios/Runner/AppDelegate.swift` line 10:
  `BackgroundService.register()`, inside `application(_:didFinishLaunchingWithOptions:)`.
- `mobile/ios/Runner/BackgroundService.swift` exists on disk (1307
  bytes) and genuinely defines `final class BackgroundService` with a
  `static func register()` — the exact symbol `AppDelegate.swift`
  calls. Not a typo or a missing method; the type is real and correct.
- `grep -n "BackgroundService" mobile/ios/Runner.xcodeproj/project.pbxproj`
  returned **zero matches** before this fix — confirmed directly, not
  inferred from the task's framing.
- Cross-checked against `AppDelegate.swift`'s own four touchpoints in
  the same file (`PBXBuildFile`, `PBXFileReference`, the `Runner` group's
  `children`, and the `PBXSourcesBuildPhase`'s `files` list) to confirm
  the expected shape of a correctly wired Swift source, and against the
  `Runner` target's `source_build_phase.files` via the `xcodeproj` gem
  (below), which listed exactly three files — `AppDelegate.swift`,
  `GeneratedPluginRegistrant.m`, `SceneDelegate.swift` — with
  `BackgroundService.swift` genuinely absent.

Diagnosis confirmed exactly as stated: the file was never added to the
Xcode project, so the compiler was never asked to compile it, so the
type is unknown at the call site. Not a Swift language bug, not a
missing import, not a stub needing implementation.

### The fix: `xcodeproj` gem, not hand-edited UUIDs

Per the task's own explicit caution, `.pbxproj` was not hand-edited.
This sandbox is Linux with no Xcode/`xcodebuild` available (confirmed:
`uname -a` reports Linux; `which xcodebuild flutter` found neither),
but the `xcodeproj` Ruby gem is pure Ruby with no Xcode dependency, so
it was installed (`gem install xcodeproj`, pulled `xcodeproj 1.28.1`
and its four dependencies from rubygems.org) and used directly against
the real project file:

```ruby
require 'xcodeproj'
project = Xcodeproj::Project.open('Runner.xcodeproj')
runner_group = project.main_group['Runner']
runner_target = project.targets.find { |t| t.name == 'Runner' }
file_ref = runner_group.new_reference('BackgroundService.swift')
runner_target.add_file_references([file_ref])
project.save
```

This generated fresh, unique UUIDs (`9869D906FD53C8C42688EE8E` for the
`PBXBuildFile` entry, `44117543AA7C8B2EE194D81C` for the
`PBXFileReference`) and added exactly the four entries
`AppDelegate.swift` already has — `PBXBuildFile`, `PBXFileReference`,
the `Runner` group's `children`, and the `Runner` target's
`PBXSourcesBuildPhase` `files` list — confirmed both by grepping the
saved file and by re-opening the saved project with the same gem
afterward and listing `runner_target.source_build_phase.files`, which
now shows all four: `AppDelegate.swift`, `GeneratedPluginRegistrant.m`,
`SceneDelegate.swift`, `BackgroundService.swift`. New UUIDs checked
against every existing ID in the file for collisions (none).

One side effect of the gem's save was caught and reverted before
committing: it cosmetically rewrote the comment label on the existing
`XCLocalSwiftPackageReference` object (from
`/* XCLocalSwiftPackageReference "Flutter/ephemeral/Packages/FlutterGeneratedPluginSwiftPackage" */`
to the shorter `/* XCLocalSwiftPackageReference "FlutterGeneratedPluginSwiftPackage" */`)
— a comment only, the underlying `relativePath` value was untouched,
but it was unrelated to this fix and reverted with a targeted `sed` to
keep the diff to exactly the four intended additions. Final diff:
`mobile/ios/Runner.xcodeproj/project.pbxproj`, 4 insertions, 0
deletions, nothing else touched.

### Checked for compounding issues before assuming this alone is sufficient

Per the task's own caution (the `mobile-android` AGP fix earlier this
session cascaded into a second, unrelated Kotlin-version error once
the first was fixed) — checked whether `BackgroundService.swift`
itself would compile cleanly once actually included, rather than
assuming target membership was the only gap:

- Its three imports (`BackgroundTasks`, `Darwin`, `Foundation`) are
  all standard system/SDK modules; none require an extra dependency,
  Swift package, or explicit "Frameworks and Libraries" linker entry
  — Swift auto-links system frameworks referenced via `import` at the
  SDK level, unlike C/Objective-C's explicit linker-flag requirement,
  and the project's own `PBXFrameworksBuildPhase` confirms no other
  system framework is explicitly listed there either (only the
  Flutter Swift package), consistent with that convention already
  being followed elsewhere in this exact project.
- `BGTaskScheduler.register(forTaskWithIdentifier: "com.onyx.sync", ...)`
  requires the same identifier to appear in `Info.plist`'s
  `BGTaskSchedulerPermittedIdentifiers` array, or the call throws at
  runtime (not a compile-time gate, but worth checking regardless).
  Checked directly: `mobile/ios/Runner/Info.plist` already has
  `BGTaskSchedulerPermittedIdentifiers` → `["com.onyx.sync"]`, matching
  the file's own `taskIdentifier` constant exactly. No plist gap.
- `runNativeSync` resolves `mobile_core_background_sync_registered` via
  `dlsym` against `RTLD_DEFAULT` (`UnsafeMutableRawPointer(bitPattern: -2)`)
  rather than a compile-time linked symbol, so a missing native symbol
  would fail softly at runtime (`guard let symbol = ... else { return 0 }`),
  not as a link error — consistent with the file's own defensive
  design, and not something target membership changes either way.

No compounding issue found. This is judged, not asserted outright,
since this sandbox has no way to run a real Swift compiler — the real
test is the CI run below.

### Verification

| Check | Result |
|---|---|
| `grep BackgroundService project.pbxproj` before fix | 0 matches (confirmed) |
| `grep BackgroundService project.pbxproj` after fix | 4 matches, matching `AppDelegate.swift`'s own 4-touchpoint shape |
| `xcodeproj` gem re-open + `source_build_phase.files` listing | `BackgroundService.swift` present alongside the other 3 Runner sources |
| UUID collision check against all existing IDs in the file | none |
| Local `xcodebuild`/`flutter build ios` | not possible — this sandbox is Linux, no Xcode toolchain (confirmed via `uname -a` and `which xcodebuild flutter`) |
| Fresh `workflow_dispatch` of `ci.yml` | see the real run's job table and log excerpt below |

Not touched, per the task's explicit exclusions: the 6 pre-existing
`api-server` integration test failures (still out of scope), and
nothing beyond this one `.pbxproj` fix — no other file in `mobile/ios/`
was modified.

### The real `workflow_dispatch` result (run `33201335832`, commit `95dbf09`)

| Job | Baseline (`33196473346`, `e0d94a7`) | This run (`95dbf09`) |
|---|---|---|
| `check` | success | success |
| `web` | success | success |
| `deploy-check` | success | success |
| `mobile-dart` | success | success |
| `native-ui-evidence` | success | success |
| `mobile-android` | success | success |
| `load-smoke` | success | success |
| `mobile-ios` | **failure** (`AppDelegate.swift:10`, `BackgroundService` not in scope) | **success** — for real, detailed below |

**Every job in this workflow is green in the same run for the first
time in this repository's CI history.**

**`mobile-ios` passes for a real reason, not a masked one.** The exact
step that always failed — `Run flutter build ios --simulator` — ran to
completion: `Running Xcode build... / Xcode build done. 91.6s`,
followed by `✓ Built build/ios/iphonesimulator/Runner.app`. One honest
caveat, checked rather than glossed over: Flutter's `flutter build ios`
suppresses `xcodebuild`'s own per-file compiler output on a
*successful* build (verbose compile lines only surface on failure, as
seen in every prior failing run's log, which did show the specific
`AppDelegate.swift:10` compiler error line-and-column). So there is no
literal `Compiling BackgroundService.swift` line to grep from this
run's log — the proof is structural, not textual: this is the exact
same command, on the exact same commit-minus-one-file-diff, that
previously failed with `Cannot find 'BackgroundService' in scope` at
this exact step, now completing and producing the real `.app` bundle,
with the `.pbxproj` compile-sources wiring being the only change in
between. Nothing else in `mobile/ios/` changed that could otherwise
explain the flip from failure to success.

**Nothing else regressed.** `mobile-dart`, `mobile-android`, and
`load-smoke` — the three jobs fixed or verified across this session's
prior two tasks — all stayed green in this same run, run back to back
with no cache-clearing or other reset in between.

## Wiring mobile Approvals to the real backend gate

### The stated diagnosis, confirmed — and one real correction to it

Directly audited before writing anything, per the task's own
instruction not to trust the framing uncritically:

- `mobile/lib/ui/screens/approvals.dart` did still show the literal
  stale text "No local Approval aggregate is registered in
  mobile-core..." — confirmed, no longer accurate.
- `mobile/lib/ui/screens/task_detail.dart` was genuinely a read-only
  debug view with zero interactive actions — confirmed.
- `mobile/lib/ui/screens/mission_detail.dart` (not part of the original
  audit, flagged as worth checking) is the exact same shape — also
  zero interactive actions.
- `TaskCommand::ApproveTask { reason: String }` / `RejectTask { reason:
  String }` confirmed directly in `crates/domains/work-domain/src/
  command.rs`. Mission's equivalents, checked rather than assumed to
  mirror Task's names: `MissionCommand::ActivateMission { reason:
  String }` (approve) and `RejectApproval { reason: String }` (reject)
  — genuinely different names, same single-`reason` payload shape.

**One real correction to the task's own framing, found by tracing the
data path rather than trusting the doc's characterization of it:** the
stale placeholder text was *literally accurate*, not just stale copy.
`controller.approvals` (`mobile/lib/ui/app.dart`) is backed by
`api.listAggregates('approval')`, which queries the local SQLite
`aggregates` table for `aggregate_type = 'approval'`
(`crates/mobile-core/src/ffi_mobile.rs`). Checked every place an
`aggregate_type` string is ever registered for a local repository
(`crates/applications/client-composition/src/app_state.rs`,
`AppStateConfig`'s repository wiring): mission, task, conversation,
message, file_asset, upload_session, policy, legal_hold,
connection_request, notification — never `"approval"`. That local
query can **never** return anything on mobile, by construction, not
because "the adapter hasn't been delivered yet" in some soon-to-change
sense.

Widening the search turned up a second, genuinely separate finding:
`api-server`'s own HTTP routes (`crates/bins/api-server/src/
routes/{mod,command,query_handler}.rs`) *do* register a real
`"approval"` repository and real `approval.Approve`/`approval.Reject`
commands, against an inline `ApprovalAggregate` struct
(`title`/`description`/`status`/`requested_by`/`target_id`/
`target_type`/...) defined right there in `routes/command.rs` — a
generic, standalone approval-request bookkeeping mechanism, explicitly
`owner_check: None` (no owner-authority gate at all), never wired into
`client-composition` (so unreachable from either `desktop-shell`'s or
`mobile-core`'s local command path). This is a real, separate concept
from Task/Mission's own approval commands — not what the stale
placeholder text was gesturing at needing "delivery," and not what
this task is about. Confirmed, then deliberately left alone: `mobile/
lib/ui/app.dart`'s `controller.approvals` field and its
`listAggregates('approval')` call were **not removed** — deciding
whether that separate `ApprovalAggregate` subsystem should ever be
wired into `client-composition` is a different, larger question this
task wasn't asked to adjudicate, and the field is simply unused now
rather than actively harmful. The real fix, instead: stop the
Approvals *screen* from depending on that always-empty query at all,
and build it against data that genuinely exists locally.

### What was built

1. **`approvals.dart` rewritten** to filter `controller.tasks` for
   `status == 'Submitted'` and `controller.missions` for `status ==
   'AwaitingApproval'` — the same local projections `TasksScreen`/
   `MissionsScreen` already load via `listAggregates('task'/'mission')`,
   confirmed real by grep against `work-domain`/`mission-domain`'s
   `state_machine.rs` (no `#[serde(rename_all = ...)]` on either enum,
   so the wire string is exactly the Rust variant name — matches
   `desktop-shell`'s own `Approvals.tsx` check of `status ===
   "Submitted"` verbatim). Each item is tappable, routing to the
   existing `TaskDetailScreen`/`MissionDetailScreen` — reusing
   `TaskCard`/`MissionCard`'s exact `Navigator.push(MaterialPageRoute(...))`
   pattern, not a new one. Empty state now reads "No tasks or missions
   are currently awaiting approval." — accurate, no longer describing
   an architectural gap that isn't the actual blocker.
2. **Real Approve/Reject actions added to `task_detail.dart`/
   `mission_detail.dart`**, not the list screen — see the placement
   decision below. Both converted from `StatelessWidget` to
   `StatefulWidget` (matching `files.dart`'s established busy/error/
   `TextEditingController` pattern) to host a reason field and
   busy/error state.
3. **One new `OnyxController.decide(...)` method** (`app.dart`),
   covering all four commands (`ApproveTask`/`RejectTask`/
   `ActivateMission`/`RejectApproval`) with one implementation rather
   than four near-identical ones, since they share the exact
   `{reason: String}` payload shape — calling
   `api.buildCommandEnvelope`/`executeCommand` exactly the way
   `createMission`/`createTask` already do, using the *target
   aggregate's own* `version`/`lifecycleEpoch`/`authorityEpoch` (it's
   mutating existing state, not creating new state) and calling
   `refresh()` afterward, matching the existing pattern exactly.
4. **A real, necessary, disclosed fix to `mobile-core`'s FFI error
   surfacing** — not part of the original ask, but load-bearing for
   requirement #3 ("a clear, specific error... not a generic
   failure"). Detailed in its own section below.

### Design decisions, explicitly resolved

**Placement: `task_detail.dart`/`mission_detail.dart`, not the
Approvals list itself.** Checked `desktop-shell`'s own equivalent
(`crates/bins/desktop-shell/ui/src/pages/Approvals.tsx` +
`components/ApprovalDialog`) before assuming, per the task's
instruction — and found it puts the action inline on its Approvals
page, not a task detail page. Deliberately *not* followed here, for a
disclosed reason found by reading desktop's own code comments: desktop
puts it there because desktop has **no backend query to list "every
task currently Submitted"** (`Approvals.tsx`'s own comment: "a real
'queue of everything Submitted' view needs a projection query the
backend doesn't have yet ... this page works against one task at a
time by id"). Mobile has no such limitation — it already holds the
full local Task/Mission list via `listAggregates`, so its Approvals
screen can be a real filtered list, and this app's own established
convention for "a list item you can act on" is tap-through-to-detail
(`TaskCard`/`MissionCard`'s existing `Navigator.push`, confirmed by
reading them). Matching *this app's own* internal convention was
judged the correct choice over copying desktop's, since desktop's
placement was itself a workaround for a constraint mobile doesn't
share.

**Reason field policy: matches `desktop-shell`'s `ApprovalDialog`
exactly** — optional for Approve/Activate, required for Reject (the
Reject button stays disabled until non-empty) — checked directly
(`reasonPolicy = { approve: { required: false }, reject: { required:
true } }`) per the task's instruction to check desktop's handling for
cross-platform consistency, and reused rather than re-decided.

**Button visibility: always shown, not hidden pre-emptively.** Checked
whether `mobile-core`'s FFI surface exposes anything the Dart side
could use to know in advance whether the current actor could
succeed — confirmed it does not (no "can this actor decide" query
exists), and building one would be new FFI surface this task didn't
ask for. Also checked desktop-shell's own precedent: `Approvals.tsx`
shows its "Review" button unconditionally too, with no advance
eligibility check. The backend fails closed either way (an empty/
unset `HierarchyCache` denies everyone), so showing-then-denying is
safe, and matches the one cross-platform precedent that exists rather
than inventing a new pattern.

### The real, necessary FFI error-surfacing fix (disclosed, not silent)

Confirmed directly, not assumed: `mobile_core_execute_command`
(`crates/mobile-core/src/ffi_commands.rs`) collapsed *every* dispatch
error — including `CommandError::OwnerAuthorityDenied`, the actual
gate this task wires up to — to a bare null pointer, identical to a
malformed/undecodable request. The file's own prior doc comment
disclosed this as a known gap ("the error itself is not currently
surfaced to the caller as anything richer than 'null'"). On the Dart
side, `OnyxMobile.executeCommand`'s `_decodeOwnedJson` turned *any*
null into `StateError('mobile-core returned null')` — genuinely
indistinguishable from a client bug. Since requirement #3 explicitly
demands "a clear, specific error... not a generic failure" for
exactly this denial path, fixing this was necessary to satisfy the
task as given, not an expansion of scope — it's the mechanism the
requirement depends on.

**The fix:** a dispatch error (envelope parsed fine, `CommandRegistry::
dispatch` itself rejected it) now serializes to `{"success": false,
"error": "<the real Display message>"}` instead of null — reusing the
same `"success"` boolean key `command_handler::handle_command`'s real
success payload already uses, so callers check one field either way,
not two different shapes. Confirmed via `CommandDispatchError`'s own
`#[error(transparent)] Decision(#[from] api_server::CommandError)`
(`client-composition/src/command_registry.rs`) that `.to_string()`
carries `OwnerAuthorityDenied`'s exact message ("actor ... is not
authorized to decide on behalf of owner ...") all the way through.
**Deliberately unchanged:** a null/invalid `handle`, an undecodable
`command_json` string, or an envelope that fails to deserialize into
`CommandEnvelope<Value>` at all still return null — these are
malformed-FFI-call bugs, genuinely different from a well-formed
command the domain layer rejected; confirmed the one existing test
covering exactly this path
(`execute_command_returns_null_for_unknown_command_type`) needed no
change, since it exercises the JSON-parse-failure branch, not
dispatch.

On the Dart side: a new `CommandFailedException` (`bridge/bridge.dart`)
carries the real message; `OnyxMobile.executeCommand` now checks
`value['success'] == false` and throws it instead of returning the
error payload as if it were a successful result. `task_detail.dart`/
`mission_detail.dart` catch it and render `error.toString()` directly
in the review card — the literal denial text, not a generic message.

**A real, pre-existing gap found and fixed along the way:**
`crates/mobile-core/expected_ffi.h` (the frozen ABI baseline
`scripts/verify/verify_ffi_signatures.sh` diffs against) was already
stale *before* this task — missing `mobile_core_set_hierarchy`/
`mobile_core_upload_file`/`mobile_core_download_file` entirely, from
earlier session work that never updated it. This script is not
actually wired into `ci.yml` (confirmed: `check`'s "Contract
verification" step runs `verify_team7.sh`/`verify_team8.sh`, not this
one), so the staleness was silently non-blocking — but updating the
baseline is purely mechanical (`cp mobile-core.h expected_ffi.h`,
exactly what the script's own comment prescribes) and fixes both the
pre-existing gap and this task's own new doc-comment diff in one
clean, disclosed step, rather than leaving it further out of sync.

### Real end-to-end proof, at the correct rigor

Per the task's own explicit standard ("mirroring the rigor of
`task_owner_authority_gate.rs`'s existing Rust-side test"), two new
tests added to `crates/mobile-core/tests/ffi_integration.rs`, going
through the *real* `mobile_core_new`/`_execute_command`/
`_set_hierarchy` FFI functions (not `CommandRegistry` in-process,
which `client-composition`'s own test already covers) — the actual
boundary Dart calls through:

- `execute_command_owner_authority_denial_surfaces_a_real_error_not_null`:
  creates a real task, drives it Draft → Ready → Active → Submitted as
  its owner, then has an unrelated stranger attempt `ApproveTask` with
  no hierarchy ever loaded — asserts the FFI call returns a non-null,
  real `{"success": false, "error": "..."}` payload whose message
  contains "not authorized to decide on behalf of", not the old bare
  null.
- `execute_command_owner_authoritys_real_manager_approves_via_ffi`:
  same setup, but loads a real hierarchy via `mobile_core_set_hierarchy`
  first (owner → manager), then confirms the owner's real,
  cache-resolved manager succeeds through the same FFI path.

A third, **pre-existing** test was found broken by this fix and
corrected: `crates/mobile-core/tests/hierarchy_authority_gate.rs`'s
`owner_submits_manager_approves_stranger_denied_through_real_ffi`
(written in an earlier session, not touched by this task's own
`ffi_integration.rs` additions) had its own `execute()` helper treat
*only* a null pointer as denial — exactly the old contract this fix
intentionally changes. Its `denied_before_hierarchy.is_err()` assertion
started failing for the right reason (a real `{"success": false, ...}`
payload is no longer `Err(())` under the old helper), confirming the
fix works, not that something broke. Fixed by teaching that test's
`execute()` helper to also treat a decoded `"success": false` payload
as a denial — every one of its existing `.is_err()`/`.expect(...)`
assertions keeps meaning exactly what it already said, with no
call-site changes needed. Re-ran clean afterward.

On the Dart side, 8 new tests in `mobile/test/unit/approvals_test.dart`
(using a `FakeOnyxApi.executeCommandOverride` hook added to
`test/fakes.dart` for driving a controlled denial without a real
native library — this sandbox has no Android/iOS runtime to link one
against): the accurate empty-state text, correct Submitted/
AwaitingApproval filtering, tap-through navigation, the Reject-disabled-
until-non-empty / Approve-always-enabled reason policy, a real
successful `ApproveTask` call with the exact envelope shape asserted
and pop-on-success, a simulated `CommandFailedException` denial
rendering its specific text without crashing the widget tree (and
*not* popping, unlike the success case), and Mission's
`ActivateMission`/`RejectApproval` command routing.

**What could not be tested in this sandbox, disclosed rather than
implied:** no real simulator/emulator exists here (confirmed: `uname
-a` reports Linux, no Xcode; Android build/test infrastructure here
builds real `.so`s and a real `.apk` but does not run either). The
Rust-level FFI tests above are the closest real substitute — they
exercise the actual compiled `mobile-core` library through its actual
C ABI, including a real SQLite database and real command dispatch, not
a mock — but a literal on-device tap-and-observe end-to-end run was
not performed, and is not claimed to have been.

### Verification checklist

| Check | Result |
|---|---|
| `cargo check --workspace` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace --no-fail-fast` | 120 test binaries `ok`; 18 failing tests, identical in name and cause to this session's already-disclosed infra limitation (no live Postgres/D-Bus in this sandbox — see the `cargo fmt` task's entry); zero of this task's own new/changed tests among them |
| `cargo test -p mobile-core` | 14 passed (11 `ffi_integration` + 2 `file_sharing` + 1 `hierarchy_authority_gate`), 0 failed |
| `scripts/verify/verify_ffi_signatures.sh` | clean after baseline update |
| `flutter analyze --no-fatal-warnings` | 0 issues |
| `flutter test` | 16 passed, 1 skipped (baseline 8 passed/1 skipped + 8 new tests, all passing) |
| `flutter build apk --debug` | real, successful assembly — `app-debug.apk`, 189.9 MB, includes the rebuilt `libmobile_core.so`/`libsync_transport_mobile.so` for arm64-v8a/armeabi-v7a/x86_64 |
| `flutter build ios --simulator` | not possible in this sandbox (Linux, no Xcode — confirmed via `uname -a`); CI's `mobile-ios` job (macOS runner) is the real verification, reported below |
| `npx tsc -b` / `npx vite build` (`admin-shell`, `web-ui`) | clean; both build |
| Fresh `workflow_dispatch` of `ci.yml` | see the real run's job table below |

Out of scope, confirmed untouched: Files, Missions, Dashboard,
Notifications, Settings screens (beyond the shared
`OnyxController.decide` addition and the `bridge.dart` exception type,
both generic infrastructure, not screen-specific logic); the separate
`ApprovalAggregate` subsystem's own fate.

### Correction: the "6 pre-existing api-server failures" label was never actually true

A user follow-up pressed on an inference I'd made without verifying it:
I had described `query_id_normalization`, `relay_switchboard`,
`staff_loan_authorization`, `staff_profile_routes`,
`team_leader_precheck_authorization`, and `user_hierarchy_admin_routes`
as "6 pre-existing, disclosed `api-server` failures" carried over from
earlier session work, and separately reported "18 failing tests" from
this task's own local `cargo test --workspace` run, without ever
reconciling the two numbers or checking whether the 6 named tests were
even among the 18.

Checked directly, twice:

1. **Local sandbox** (`/tmp/cargo_test_workspace3.log`, this task's own
   run): grepping for all 6 test files by name shows all 6 ran and
   **all passed** — `query_id_normalization`: 4/4 ok,
   `relay_switchboard`: 3/3 ok, `staff_loan_authorization`: 3/3 ok,
   `staff_profile_routes`: 8/8 ok, `team_leader_precheck_authorization`:
   3/3 ok, `user_hierarchy_admin_routes`: 8/8 ok. The 18 local failures
   are a completely disjoint set from these 6 (0 overlap) — they are
   Postgres/D-Bus-dependent tests failing for lack of that
   infrastructure in this local sandbox, same root cause already
   disclosed elsewhere in this file, unrelated to the 6 named tests.

2. **Real CI**, run `33279194057` (commit `9f4ed77`, `check` job, job
   id `99171315636`, the "Test" step, with real Postgres + a real D-Bus
   session confirmed by the job's own preceding "Run database
   migrations" and "Start a D-Bus session..." steps):
   pulled the literal log content (not the job's aggregate
   success/failure conclusion — the actual per-test lines) and
   confirmed the same result: all 6 files ran, every individual test in
   all 6 passed, 0 failures, e.g. `test result: ok. 4 passed; 0 failed`
   for `query_id_normalization`, `... 8 passed; 0 failed` for both
   `staff_profile_routes` and `user_hierarchy_admin_routes`, etc.

**Conclusion:** nothing in this session's mobile-Approvals work (or any
other change) "fixed" these 6 tests — they were never actually broken
in a properly provisioned environment. The "pre-existing failure" label
was carried forward from an earlier report without ever being
confirmed against real infrastructure; it was itself an artifact of the
same class of problem as the 18 local-only failures (missing
Postgres/D-Bus in whatever sandbox first produced that label), not a
real, load-bearing defect. This is now corrected: there are no known
`api-server` test failures against real infrastructure as of this
commit.

### Real CI run: full job table

`workflow_dispatch` run `33279194057`, commit `9f4ed77`, compared
against the prior real run `33201335832`, commit `95dbf09`:

| Job | Baseline (`95dbf09`) | This run (`9f4ed77`) | Regression? |
|---|---|---|---|
| mobile-dart | success | success | no |
| web | success | success | no |
| check | success | success | no |
| deploy-check | success | success | no |
| mobile-android | success | success | no |
| mobile-ios | success | success | no |
| native-ui-evidence | success | success | no |
| load-smoke | success | success | no |

All 8 jobs green, no regressions. `mobile-android`'s `flutter build apk
--debug` and `mobile-ios`'s `flutter build ios --simulator` (the two
jobs this sandbox itself cannot run) both completed successfully on
their real runners, confirmed via each job's own step list rather than
inferred from the run's overall conclusion.

## Hardening H1, H2, H4(a) — production bootstrap, distributed session revocation, CORS fix

Three of six agreed hardening tracks from an independent production-
readiness audit (H3/relay-ticket auth and H4(b)/transport-TLS are
separate follow-up tasks; H5/H6 are out of scope here). Every diagnosis
below was re-confirmed directly against the real source at the current
`main` tip before any code changed, not assumed from the task text.

### H1 — Production bootstrap

**Diagnosis confirmed.** A deliberately authorized development shortcut
currently seeds a known administrator credential on an empty database.
The implementation clearly documents the tradeoff, so this is not an
undisclosed implementation error. However, the exception is not
sufficiently isolated from production execution and therefore remains a
production release blocker.

Concretely: `ApiState::new` (`crates/bins/api-server/src/routes/mod.rs`)
seeded `"All-Father"` / `"passvord0000"` whenever `user_store.count()
== 0`, with no environment check at all — a fresh production install is
also an empty database, so production got exactly the same shortcut as
local dev. The token-gated `POST /api/admin/bootstrap` flow
(`ONYX_BOOTSTRAP_TOKEN`, `routes/admin.rs`) was never touched by the
original change and still works correctly; it was simply never the path
actually exercised, since the seed always won the race by running first
in `ApiState::new`.

**Fix.** The seed now reads `if environment != "production" &&
user_store.count().await? == 0`. `ONYX_ENV=production` categorically
refuses to seed, full stop — and the function's own pre-existing gates
already guarantee that a real production boot has a genuine Postgres
primary, a real `ONYX_AUTHORITY_SIGNING_KEY`, and a real
`ONYX_GOVERNANCE_DATABASE_URL`, so production's only remaining path to a
first admin is the untouched, token-gated `/api/admin/bootstrap` flow —
restored to being the authoritative production path rather than a
fallback nothing production-side ever exercised.

**New test.** `tests/end-to-end/production_bootstrap.rs`
(`production_env_never_seeds_the_known_admin_account`): boots a real
`ApiState` with `ONYX_ENV=production` and a genuinely empty database
(backed by a real, ephemeral Postgres container via the existing
`PostgresHarness`/testcontainers harness, exercising the same
production-only gates a real deployment would hit), asserts `SELECT
COUNT(*) FROM users` is `0` afterward, then asserts a login attempt with
the known `"All-Father"` / `"passvord0000"` credentials returns 401. This
is the test that proves the fix, not just that the gating line changed.

### H2 — Distributed session revocation

**Diagnosis confirmed, and one part of the task's own framing corrected.**
`revoked_tokens: Arc<RwLock<HashSet<String>>>` was real, in-process,
per-instance memory — genuinely incompatible with a multi-replica
deployment, exactly as described. However, checking every call site
directly (not assumed) found that only `logout` and refresh-token
rotation ever touched it. `deactivate_user` and `set_user_password`
(`routes/admin.rs`) did **not** touch it at all — their own existing doc
comments said so explicitly ("existing tokens for this user remain valid
until they expire... the in-memory revocation set cannot express 'revoke
all tokens for a user' across pods"). So this fix isn't just "move four
existing call sites to a shared store" — two of the four never had any
revocation behavior to move, and had to be wired up for the first time.

**Design decision: both models, not one.** The task asked for an explicit
choice between individual-token tracking and session/family tracking.
The honest answer is that a single model doesn't cover both real call
sites:
- `logout` and refresh-token rotation each hand back **one** specific
  token they hold. Individual-token revocation (a hash of the token,
  looked up on every `validate_token` call) matches this exactly and
  changes nothing about existing behavior.
- `deactivate_user` and `set_user_password` need to invalidate **every**
  session a user currently has, and the server has never tracked which
  individual tokens are outstanding for a user (no session table, no
  enumeration). Individual-token tracking cannot express this without
  retroactively building that tracking. A per-user watermark
  (`user_token_revocations.revoked_before`, compared against each
  token's own `iat` claim) expresses "everything issued before now is
  dead" in one write, independent of how many sessions exist.

Implemented as a new `TokenRevocationStore` port
(`security_application::ports::token_revocation`) with both operations,
backed by `PostgresTokenRevocationStore`
(`security-adapter/src/token_revocation.rs`) — real, shared, durable,
selected by `ApiState::new` via the exact same governance-pool-then-
primary-pool precedence `PostgresSlidingWindowRateLimiter` already uses.
An `InMemoryTokenRevocationStore` fallback exists only for a pure-
SQLite, single-instance, no-governance-database composition (local dev),
carrying the same disclosed non-durability the old field had everywhere
— now honestly scoped to the one topology where it's harmless. Production
can never reach it: `ONYX_ENV=production` already requires a Postgres
primary and `ONYX_GOVERNANCE_DATABASE_URL`, both of which route to
`PostgresTokenRevocationStore`. Two new tables
(`20260109000000_add_token_revocation.{up,down}.sql`, both migration
sets, mirroring the existing `rate_limit_events` migration's dual-
directory convention): `revoked_tokens (token_hash, revoked_at)` and
`user_token_revocations (user_id, revoked_before)`. Tokens are hashed
(SHA-256) before storage, never stored raw. `validate_token` now checks
both: an individually-revoked hash, and the caller's per-user watermark
against the token's `iat`.

**New tests**, both in `tests/end-to-end/`, both proving the actual
production-topology property (a revocation performed on one replica must
be visible to a second, independent replica that shares nothing but the
database — the exact thing an in-process `HashSet` could never satisfy):
- `session_revocation.rs`'s
  `logout_on_one_replica_revokes_the_token_on_a_second_independent_replica`:
  two separate `ApiState`/`Router` instances against one real Postgres
  database; confirms the token works against replica B before logout (so
  the later rejection is attributable to the revocation, not a generic
  auth failure), logs out via replica A only, then confirms replica B
  rejects the same token.
- `deactivating_a_user_on_one_replica_revokes_their_session_on_a_second_replica`:
  same two-replica structure, proving the per-user watermark path
  (previously nonexistent) is genuinely shared too, not just the
  single-token path.

### H4(a) — CORS fix

**Diagnosis confirmed, and the task's own hint to "check for real" paid
off.** `allow_methods([GET, POST, OPTIONS])` was indeed missing `PUT`
for `/api/admin/mobile-access`'s `.put(admin::set_mobile_access)` as
described — but auditing every `.route(...)` call in the router (not
assuming that was the only one) found a **second**, undisclosed PUT
mismatch: `/api/admin/profiles` (`put(profiles::upsert_profile_route)`)
is a PUT-only route with no GET/POST alternative at all, so it was
completely unreachable cross-origin from a browser, not merely missing
one verb alongside others. Every other registered route in this router
uses GET or POST only — confirmed by reading the whole router body, not
sampled.

**Fix, part 1 (small, mechanical):** added `Method::PUT` to
`allow_methods`, covering both real routes.

**Fix, part 2 (origin allow-list — the real open question):** moved off
`allow_origin(Any)` toward an explicit, config-driven allow-list
(`ONYX_CORS_ALLOWED_ORIGINS`, comma-separated), required and validated in
production. The concrete origin list could not honestly be hardcoded:
checked this repo's actual deployment config directly
(`deploy/helm/`, `deploy/docker/`) and confirmed neither `web-ui` nor
`admin-shell` has a Dockerfile, Helm chart, or ingress entry anywhere —
both exist today only as local Vite dev servers (ports 5173 and 5174
respectively). Only `api-server` itself is deployed
(`deploy/helm/onyx-api`, `api.onyx.example.com`). Since no real
production origin for either browser client has been decided or
deployed yet, this is genuinely a deployment-time config decision, not
something the code can guess — `ApiState::new` refuses to boot in
production without `ONYX_CORS_ALLOWED_ORIGINS` set to at least one valid
origin, with an error message explaining exactly why, rather than
silently falling back to permissive or to an invented placeholder
domain. Outside production the permissive `Any` default is unchanged, so
every existing local dev/test workflow (both Vite dev servers,
desktop-shell's webview, mobile emulators, the `web`/`native-ui-
evidence` CI jobs) keeps working exactly as before.

**Note on numbering:** this task's H1/H2/H4(a) labels are a task-local
renumbering. This repository's own `docs/AUDIT_REGISTER.md` calls these
same three items H-01 (bootstrap portion)/H-02/H-03 respectively; its H-04
is an unrelated dependency-currency finding, not this task's H4(b). Both
numbering schemes are preserved verbatim in code comments/migration
names where each already existed, to avoid erasing either trail.

### Verification

| Check | Result |
|---|---|
| `cargo check --workspace` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean (after one `cargo fmt` pass on the 3 new files) |
| `cargo test -p api-server --release` | 31 passed, 0 failed — includes all 6 previously-verified tests, confirmed still passing, zero regression from the `validate_token`/`logout`/`refresh`/`deactivate_user`/`set_user_password` rewiring |
| `cargo test --workspace --release --no-fail-fast -- --test-threads=1` (real D-Bus/gnome-keyring session started locally, matching CI's own setup step exactly, including the `--components=secrets --daemonize` unlock this session's own keyring tests need) | 557 passed, 19 failed — every failure is a disclosed, pre-existing infra gap this local sandbox cannot provide: 7 from `crates/team8-e2e-tests` (testcontainers needs a live Docker daemon; `docker info` confirms none is running here) and 12 from `persistence-postgres`'s own direct-Postgres integration tests (`DATABASE_URL must be set`, requiring the real Postgres service only CI's `check` job provisions) — zero unexplained or newly-introduced failures |
| H1's new test (`production_bootstrap.rs`) | fails locally only because it also needs the same live-Postgres testcontainers harness as the e2e crate (`Socket not found: /var/run/docker.sock`) — cannot be verified in this sandbox; will run for real in CI, where GitHub-hosted runners provide Docker natively (same reasoning `tests/end-to-end`'s pre-existing journeys already rely on) |
| H2's two new tests (`session_revocation.rs`) | same testcontainers/Docker limitation as above — not verifiable in this sandbox, will run for real in CI |
| Fresh `workflow_dispatch` of `ci.yml` | see the job table below |

Out of scope, confirmed untouched: H3 (relay topology, relay-ticket
auth), H4(b) (transport/TLS enforcement), H5 (Docker lockfile
reproducibility), H6 (mobile CI immutability/native acceptance gates).

### Real CI caught a real bug: off-by-one in H2's per-user watermark

The first `workflow_dispatch` of the above (run `33289176310`, commit
`540682c`) genuinely exercised the two new tests' Docker-backed Postgres
path for the first time (this sandbox cannot run testcontainers at all —
see above) and found a real defect, not a sandbox artifact:
`session_revocation.rs`'s cross-replica deactivation test failed —
`left: 200, right: 401` — while `logout_on_one_replica_revokes_...` and
`production_bootstrap.rs` both genuinely passed in that same run.

Root cause: `validate_token`'s watermark check was `claims.iat <
revoked_before`, and both values come from `unix_seconds()` — 1-second
resolution. The test logs in and calls `deactivate_user` fast enough
that both timestamps land in the same second, so `iat == revoked_before`
and the strict `<` treated the token as still valid. Fixed by changing
the comparison to `<=` (`crates/bins/api-server/src/routes/mod.rs`) —
fail-closed on the tie. The only cost is a legitimate caller who logs in
again within the same second a deactivation/password-reset watermark was
set getting one extra rejected request; the alternative (a token that
should be dead staying valid) is the actual security property H2 exists
to close, so the tradeoff is the right one. Pushed as `67ebf8b`; a second
`workflow_dispatch` re-verifies both new tests for real against this fix.

## Hardening H3 (relay topology isolation) and H4(b) (transport security)

Two of the audit's remaining tracks, deliberately scoped together because both
touch `crates/bins/api-server/src/routes/relay.rs` (H1/H2/H4(a) landed
separately as commit `540682c`; H5/H6 remain future work).

### Discrepancy resolved first: "3-20" vs "5-30" replicas

The audit cited "3-20 replicas"; a direct check of
`deploy/helm/onyx-api/values-production.yaml` found `replicaCount: 5` and
`autoscaling: {minReplicas: 5, maxReplicas: 30}` instead. Both numbers are
real and current, not a stale audit figure: `deploy/helm/onyx-api/values.yaml`
(the base chart's own defaults, read directly rather than assumed) has
`replicaCount: 3` / `autoscaling: {minReplicas: 3, maxReplicas: 20}` --
exactly the audit's figure. `values-production.yaml` is an environment
overlay applied via `-f values-production.yaml` at deploy time, raising the
production numbers to 5/5-30. Neither file is wrong; they are different
layers of the same Helm chart. As anticipated, the exact number does not
change either fix below -- what matters is "more than one replica,
autoscaled," which is true under either figure.

### H3 -- Relay topology isolation

**Diagnosis confirmed.** `RelayRegistry` (`routes/relay.rs`) is genuinely
in-memory, per-process peer presence with no cross-process forwarding. Two
users landed on different `onyx-api` replicas could not reach each other over
Cloud Relay -- a silent correctness failure, not a crash, exactly as
described.

**Code-coupling check performed, not assumed.** Read `relay_route` and
`serve_relay` in full before deciding: their only real dependencies are
`ApiState::relay_registry`, `ApiState::secret_provider`, and
`ApiState::token_revocation_store` (via `validate_token`) -- narrow. But
`ApiState::new` unconditionally constructs the *entire* application state on
every boot (every repository, migrations, the rate limiter, the audit
writer, the password hasher, the user store) with no lighter-weight
constructor, and `router()` wires every route into one `Router`
unconditionally. Splitting relay into a genuinely separate binary would mean
introducing a second, parallel "minimal ApiState" construction path and a
second router -- real new surface area, and a second thing to keep in sync
with every future `ApiState` field. That is a bigger, riskier change than
"immediate containment" calls for.

**Design chosen: same binary, separate Helm release, decoupled routing.**
New chart `deploy/helm/onyx-api-relay/` reuses the exact `onyx-api` container
image (confirmed via `deploy/docker/api-server.Dockerfile` -- no new
Dockerfile needed) but is deployed as its own release:

- `templates/deployment.yaml` hardcodes `replicas: 1` directly in the
  template -- not `{{ .Values.replicaCount }}` -- and the value is not
  exposed in `values.yaml` at all. This is deliberate: a plain Deployment
  (no Argo `Rollout`, no HPA, no canary) whose replica count cannot be
  changed by any values override, environment file, or `helm upgrade --set`,
  because there is nothing to override. Uses `strategy: {type: Recreate}`
  rather than the default `RollingUpdate`, so a deploy never briefly runs two
  pods of this Deployment (which the default 25% `maxSurge` would round up to
  for a single-replica Deployment) -- a short full outage of relay during
  deploys, in exchange for the single-replica invariant genuinely never being
  violated even transiently.
- A second `Ingress` object (`templates/ingress.yaml`) on the *same* host as
  `onyx-api`'s own ingress, carrying only the `/api/relay` path.
  nginx-ingress-controller merges every Ingress object for a given host into
  one compiled routing table and matches by path specificity regardless of
  which Ingress resource (or Helm release) a rule came from, so `/api/relay`
  always wins over `onyx-api`'s own `/` catch-all -- this is what actually
  routes relay WebSocket traffic to the dedicated pod, not an assumption that
  clients dial the right place. `/api/relay-ticket` (new, see H4(b) below) is
  deliberately a sibling path, not a child of `/api/relay`, specifically so
  this Prefix rule does not also catch ticket-minting traffic and pull it off
  the scaled, autoscaled `onyx-api` fleet where it belongs (minting is
  stateless and cheap).
- `RelayRegistry`'s own doc comment (`routes/relay.rs`) now states this is
  the actual, enforced production topology, not an aspirational note about a
  known limitation -- corrected as part of this change, not left stale.

**Deferred, explicitly, not built:** the long-term fix is shared presence
plus inter-node pub/sub (Redis or NATS) so relay itself can run more than one
replica. Moving `RelayRegistry` into Postgres alone -- the audit's own
rejected alternative -- would only have fixed presence *discovery*, not
actual cross-process WebSocket frame forwarding, which needs a real message
bus between nodes. Not attempted here; this task is containment only.

**Real verification, not a YAML review.** Installed Helm 3.15.3 (matching
`ci.yml`'s pinned version) and actually rendered the chart:
- `helm template ... --set replicaCount=5` on `onyx-api-relay` still renders
  `replicas: 1` -- confirmed live, not just claimed: the override has zero
  effect because the value was never wired to anything.
- Confirmed exactly one `Deployment` and zero `HorizontalPodAutoscaler`
  objects render from the chart.
- Rendered `onyx-api` with `-f values-production.yaml` and confirmed its own
  HPA still renders with `minReplicas: 5` / `maxReplicas: 30`, unaffected by
  the new chart's existence -- ordinary API replicas keep scaling
  independently.
- `helm lint` passes on the new chart.
- All of the above is now also a real CI step (`deploy-check` job, "Verify
  the relay chart genuinely renders single-replica (H3)"), not just something
  run once locally.

### H4(b) -- Transport security

**Item 1: plaintext HTTP for non-loopback Admin connections.**

Confirmed two real, independent save paths for the server address, not one:
`Login.tsx`'s `ConnectionSettings` component and `Settings.tsx`'s
`ServerConnectionSettings` -- both fixed. Checked for an existing client-side
equivalent to the server's `ONYX_ENV` first, rather than assuming one exists
or inventing a new one blind: none exists anywhere in `admin-shell/ui`. One
was not needed, though -- Vite's own, already-real build-mode flag,
`import.meta.env.PROD`, already exactly tracks "is this the packaged,
distributed app" vs. "a local `npm run dev` / `tauri dev` session," since
`package.json`'s `build` script (what `tauri build` invokes to produce the
real shipped app) always runs `vite build`, which sets `PROD = true`
unconditionally regardless of `--mode`. Introducing a parallel
`ONYX_ENV`-style variable would just be a second flag carrying the same
meaning Vite already provides for free.

`isSecureEnoughForProduction()` (`utils/serverAddress.ts`): allows any
`https://` address, allows `http://` only to `127.0.0.1`/`localhost`/`::1`,
rejects every other `http://` address, and is a no-op (returns `true`
unconditionally) outside `import.meta.env.PROD` so every local dev/test
workflow is unchanged. Wired into both save paths (reject before the health
check even runs) and, as a backstop, into `api/client.ts`'s request
interceptor -- covering an address saved by a build predating this check, or
one edited directly in `localStorage`, not just the two UI save flows.

**Real verification, not a code read.** No test framework exists in
`admin-shell/ui` at all (no vitest/jest, no `test` script in `package.json`)
-- confirmed by searching, not assumed; adding one for a single check would
be a real scope expansion beyond this task. Instead ran the actual production
build (`npm run build` -- real `tsc -b && vite build`, the same command
`tauri build` invokes) and inspected the emitted bundle directly:
`dist/assets/client-*.js` contains the compiled `isSecureEnoughForProduction`
with the `if (!import.meta.env.PROD) return true` branch entirely absent --
Vite statically evaluated `import.meta.env.PROD` to the literal `true` for
every `vite build` invocation and dead-code-eliminated the bypass, proving
the guard is unconditionally live in the real, shipped artifact, not merely
reachable in theory. `tsc -b` (part of the same build command) and `oxlint`
both pass clean.

**Item 2: relay auth token in the WebSocket query string.**

Confirmed present exactly as described: `/api/relay/:target_id?token=...`
put the real, hour-long, full-API-scope bearer access token in a URL.
Replaced with a purpose-built relay ticket:

- **New route**: `POST /api/relay-ticket` (`routes/relay::issue_ticket`),
  authenticated normally (`authenticate_headers`), takes `{"target_id": ...}`.
  Deliberately a normal, stateless route left on the scaled `onyx-api` fleet
  (see the Ingress placement above) -- it reads only the shared JWT signing
  key every replica already has, so it does not need the single relay
  replica the WebSocket upgrade itself requires.
- **Lifetime**: 30 seconds (`RELAY_TICKET_TTL_SECONDS`), enforced by the
  exact same `exp` check `validate_token` already applies to access/refresh
  tokens -- long enough to cover minting-then-connecting over a real network
  including one retry, short enough that a leaked ticket is nearly worthless
  by the time anyone could act on it.
- **Scope**: bound to the specific `target_id` requested, carried in the
  reused `TokenScope::object_id` field. `relay_route` rejects a ticket
  whose `object_id` does not match the path segment actually dialled --
  proven directly by
  `relay_ticket_cannot_be_used_against_a_different_target` (new test).
- **Single-use**: enforced by `RelayRegistry::redeem_ticket_once`, keyed on
  the ticket's own `jti`. This is intentionally process-local, in-memory
  state -- which would have been the wrong choice before this task (see H2's
  reasoning for `revoked_tokens` in the earlier entry), but is the *correct*
  choice here specifically because H3 (above) now guarantees exactly one
  relay process exists: there is no cross-replica redemption race to defend
  against, because there is only ever one replica. Proven directly by
  `relay_ticket_cannot_be_redeemed_twice` (new test): an identical, unexpired,
  correctly-scoped ticket is refused the second time it is presented.
- Reuses the existing `TokenClaims`/`Ed25519JwtCodec` machinery rather than
  inventing a parallel token format, with a new `token_type` discriminator
  (`"relay_ticket"`) so a ticket can never be accepted where an access/refresh
  token belongs or vice versa. `validate_token`'s existing revocation checks
  (`is_token_revoked`, `user_revoked_before`) apply to tickets for free, so a
  deactivated user's in-flight ticket is also correctly invalidated.

**Real client fix, not server-only.** The only real, shipping relay client in
this codebase is `desktop-shell`'s `TungsteniteRelaySocketFactory`
(`mobile-core`'s equivalent is still `NotYetImplementedSocketFactory`, a
placeholder -- confirmed, nothing to fix there). Updated it to call
`POST /api/relay-ticket` over real HTTPS (deriving the ticket endpoint's host
from the relay's own WebSocket URL, swapping `wss`/`ws` for `https`/`http`)
before dialling the WebSocket, and to send the returned ticket, not the raw
bearer token, as the `ticket=` query parameter.

### Verification

| Check | Result |
|---|---|
| `cargo check --workspace` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `cargo test -p api-server --release` | 33 passed, 0 failed -- includes all 6 previously-verified tests and both of H1/H2's prior new tests (via the shared harness in the earlier task), plus 2 new relay-ticket tests, with zero regressions |
| `cargo test --workspace --release --no-fail-fast` (real D-Bus/gnome-keyring session) | 559 passed, 19 failed -- the identical, already-disclosed set from the prior task (7 testcontainers/Docker-dependent e2e tests, 12 `persistence-postgres` direct-Postgres tests), zero new or unexplained failures |
| `helm lint deploy/helm/onyx-api-relay` | clean |
| `helm template` single-replica proof (see H3 above) | confirmed live, both locally and as a new CI step |
| `npm run build` (admin-shell/ui) | clean; production bundle inspected directly and confirms the H4(b) item 1 guard is unconditionally compiled in |
| `oxlint` (admin-shell/ui) | clean |
| Fresh `workflow_dispatch` of `ci.yml` | see the job table in this task's chat report |

Out of scope, confirmed untouched: the long-term shared-bus relay redesign
(documented above as deferred), H5 (Docker lockfile reproducibility), H6
(mobile CI immutability/native acceptance gates), and `/api/events`'s own,
separate `?token=` query parameter (a different route, not part of this
task).

## Hardening H5 (deterministic release builds) and H6 (CI immutability + native acceptance gates)

The last two of six hardening tracks from the production-readiness audit.
H1/H2/H4(a) (`540682c`/`67ebf8b`/`2024dbe`) and H3/H4(b) (`57b826e`) are both
already done and CI-confirmed; this task is independent of both (no code
overlap -- confirmed by reading the diffs, not assumed) and closes the audit
out.

### H5 -- Deterministic release builds

**Diagnosis confirmed, and it turned out to run deeper than the audit's
single cited example.** Direct grep confirmed the buggy
`cargo generate-lockfile && cargo build --locked --release` pattern in all 5
Dockerfiles (`api-server`, `desktop-shell`, `migration-tool`, `sync-agent`,
`worker`) -- self-defeating, since regenerating the lockfile immediately
before a `--locked` build means `--locked` can never catch drift against a
lockfile it just wrote itself. The exact same pattern was also present in
`scripts/release.sh` (line 19, immediately before its own `--locked` build)
-- not mentioned in the task text, found by reading the script in full while
tracing what `verify_team8_static.py`'s second check (below) actually
validated.

**Fix:** removed `cargo generate-lockfile &&` from all 5 Dockerfiles, which
now build `cargo build --locked --release -p <crate>` directly against
`Cargo.lock` as committed (already present via `COPY . .`). `release.sh`'s
`cargo generate-lockfile` was replaced with a read-only
`cargo metadata --locked` check (see below) rather than simply deleted, so a
stale lockfile is caught with a clear error before a multi-minute release
build starts, instead of failing confusingly partway through or (worse)
silently succeeding against freshly-rewritten dependencies.

**`verify_team8_static.py`'s two checks, handled as the task asked --
distinctly, not blanket-removed:**

- **Line ~100** (Docker pattern): previously required the literal buggy
  string to be present -- inverted to require the corrected pattern
  (`cargo build --locked --release` present, `cargo generate-lockfile`
  absent) in every Dockerfile body.
- **Line ~267-268** (release workflow/script pairing): read this one's full
  context before touching it, per the task's explicit instruction, since it
  visibly combined two different files. Confirmed `release_workflow`
  (`.github/workflows/release.yml`)'s own `cargo metadata --locked --no-deps`
  gate was already present in all three of its build jobs, ahead of their
  own `--locked` builds -- genuinely correct on first read. The
  `release_script` half required `scripts/release.sh` to literally contain
  `cargo generate-lockfile` -- which is exactly the same bug being fixed
  elsewhere, just required by the verifier as if it were a correctness
  property. Not blanket-removed: replaced with a requirement that
  `release.sh` contain the same corrected `cargo metadata --locked` gate
  instead, so the check now enforces the fixed pattern in both files rather
  than the bug's presence in one of them.

**A second, more consequential bug found by actually testing the "already
correct" gate, not by reading it.** Before concluding `cargo metadata
--locked --no-deps` was a legitimate check worth preserving, it was tested
directly: pinned a workspace dependency (`anyhow`) to an exact version the
committed `Cargo.lock` cannot satisfy, then ran the exact command. It exited
`0` -- passed -- despite genuine, real drift. `--no-deps` skips full
dependency-graph resolution entirely, so `--locked` has nothing left to
validate against; `cargo metadata --locked` (same command, `--no-deps`
dropped) correctly failed the identical test with a real
`failed to select a version for the requirement` error. This means the
"legitimate" gate the task described was itself silently non-functional
everywhere it existed: all three `release.yml` build jobs, and a third file
not mentioned in the task at all -- `.github/workflows/Debug.yml` (a manual,
Windows-only debug-build workflow), which had the identical broken
`--no-deps` pattern in three more places. Fixed by dropping `--no-deps` in
every one of these locations (`release.yml` x3, `Debug.yml` x3,
`scripts/release.sh`, plus the new `ci.yml` step below), and updated
`verify_team8_static.py`'s line-267 check accordingly (requires the base
string present and the broken `--no-deps` variant absent, in both files).

**`ci.yml` had no lockfile-drift gate at all.** Confirmed by grep before
assuming otherwise: `cargo metadata --locked` appeared nowhere in `ci.yml`.
A lockfile drifted out of sync with `Cargo.toml` on an ordinary push/PR was
never caught there -- only at actual release time, if at all (given the
`--no-deps` bug above, not even then). Added a "Verify locked dependency
graph" step to the `check` job, placed before Clippy/Build so a stale
lockfile is reported as exactly that rather than a confusing downstream
compile error.

**`cargo sqlx prepare --check`, added per the original plan.** This repo
carries committed `.sqlx/` offline query metadata (`SQLX_OFFLINE=true` in
`ci.yml`), so a query that changes without regenerating that metadata would
previously build successfully against the stale cache with no warning.
Added `cargo sqlx prepare --check --workspace -- --all-targets` (after
`sqlx-cli` installation and after the `check` job's own migration step,
since unlike the workspace build itself this genuinely needs a live,
migrated schema to check queries against).

**Real verification, both properties, not assumed:**
- Installed Helm... no -- installed a real local PostgreSQL 16 (already
  present in this sandbox's apt cache) and `sqlx-cli`, ran migrations, then
  ran `cargo sqlx prepare --check` against the real, current `.sqlx/`
  metadata: passed silently (exit 0, the correct behavior on a match).
  Deleted one `.sqlx/query-*.json` file to simulate real drift and reran:
  failed with `.sqlx is missing one or more queries; you should re-run sqlx
  prepare` (exit 1) -- restored the file afterward, confirmed `git status`
  clean.
- Deliberately staled `Cargo.lock` (pinned `anyhow = "=1.0.200"` in the
  workspace manifest, a version the lock cannot satisfy, without touching
  `Cargo.lock`): `cargo metadata --locked` (the corrected command, no
  `--no-deps`) failed with a real, specific error naming the unsatisfiable
  requirement; reverted with `git checkout --` and confirmed clean again.
  This is the actual property H5 exists to guarantee, tested directly
  rather than inferred from the Dockerfiles looking different.
- `python3 scripts/verify/verify_team8_static.py`: 363/363 checks pass
  against the fixed files.

### H6 -- CI immutability (mobile) + native acceptance gates

**Part 1 (done). Diagnosis confirmed exactly as described** in `ci.yml`'s
`mobile-dart` job: `flutter pub upgrade` ran immediately after `flutter pub
get`, silently bumping every dependency to the latest version its
constraints allowed before anything was validated -- a green run proved the
*upgraded* tree passed, not the tree actually committed in
`pubspec.lock`. `dart fix --apply || true` ran after that, mutating source
in place, with `|| true` additionally swallowing any failure from the fix
step itself so CI never reported whether it even succeeded.

Removed both. Installed a real Flutter 3.47.2 SDK in this sandbox (not
previously present) to verify the replacement job for real rather than by
inspection:
- `flutter pub get` alone (no `upgrade`) installs cleanly from the committed
  `pubspec.lock`.
- `flutter analyze` (no `dart fix` beforehand) reports **zero issues**
  against the current tree -- confirmed directly, not assumed, before
  deciding whether fatal warnings were safe to enable. Since there is no
  existing backlog, making warnings fatal (dropping `--no-fatal-warnings`)
  cannot turn CI red for anything unrelated to a given change, so it was
  enabled outright rather than deferred. This also directly confirms the
  task's premise: `flutter analyze` alone is sufficient to catch real
  issues -- this session's own earlier `mobile/lib/net/auth.dart` parser
  ambiguity was caught by `flutter analyze` directly, never by an auto-fix
  step, which only ever mutates, never diagnoses.
- `flutter test`: 16 passed, 1 skipped (the same pre-existing device-lab
  skip noted below), unaffected by removing the mutation steps.
- `bash scripts/verify/verify_mobile.sh`: passes.

**Part 2 (scoping only, per the task -- nothing built).** Read all three
currently-`#[ignore]`d native journeys in full, not just their attribute
text:

- `tests/end-to-end/p2p_sync.rs` (`journey_6_p2p_sync`) --
  `#[ignore = "requires signed Team 5 desktop/mobile clients and radio
  adapters"]`. Empty body (`{}`) -- a reserved slot, not a partially-blocked
  test.
- `tests/end-to-end/background_sync.rs` (`journey_7_background_sync`) --
  `#[ignore = "requires Team 5 iOS BGTask and Android WorkManager release
  builds"]`. Also an empty body.
- `tests/end-to-end/notification_sync.rs` (`journey_5_notification_sync`) --
  `#[ignore = "Team 5 client event integration is not production-complete"]`,
  with its own doc comment citing "Team 8 ruling R11: keeps client-dependent
  journeys ignored until Team 5 native-client completion and release signing
  are available." Also an empty body; its own comment notes the backend
  half (command/query flow) is already covered elsewhere (Team 6 integration
  tests) -- this journey is specifically the *client* half.

The mobile-side counterpart, `mobile/test/integration/p2p_sync_test.dart`,
is instructive about how far the scaffolding for this already goes: it
gates on `ONYX_MOBILE_DEVICE_TEST=1` and, even when that variable is set,
its body only calls a fake `triggerSync()` against a mocked API -- the
env-var gate exists as a naming convention for a future real device-lab
run, not a working one today.

**Honest assessment, per journey:**

1. **`journey_6_p2p_sync` (Wi-Fi Direct / BLE).** Highest cost, most
   infrastructure-heavy of the three. These radios cannot be virtualized in
   any meaningful way -- a cloud CI runner has no Bluetooth/Wi-Fi Direct
   hardware at all, and there is no credible emulator substitute for actual
   short-range radio behavior (discovery, pairing, real-world interference,
   range). Converting this into a real gate needs a physical device lab:
   at minimum two real phones (one iOS, one Android, per the ignore reason)
   on a persistent bench or a managed device-farm service with radio
   support (most cloud device farms, e.g. Firebase Test Lab, explicitly do
   not support BLE-to-BLE or Wi-Fi Direct pairing between two rented
   devices in the same session -- this would likely require an
   in-house/self-hosted lab, not a SaaS farm). Feasibility: low without a
   real hardware investment and ongoing maintenance (device OS updates,
   battery/charging management, physical security); the test itself is
   also going to be flakier than anything running purely on CI hardware,
   since it's exercising real radio conditions.
2. **`journey_7_background_sync` (iOS BGTask / Android WorkManager).**
   Medium cost. Android WorkManager behaves reasonably well in an emulator
   for basic scheduling, but real fidelity (Doze mode, battery-optimization
   task killing, actual background execution windows) is only fully
   trustworthy on real hardware. iOS's BGTaskScheduler is the harder half:
   the iOS Simulator does not fire background tasks on its own timeline at
   all -- triggering one requires either a real device or manually invoking
   the task via a debugger/`simctl` command, which tests *that the handler
   runs when invoked*, not that the OS actually schedules and fires it
   under real conditions. A CI-hosted acceptance gate could plausibly cover
   the "handler executes correctly when triggered" half on
   simulators/emulators relatively cheaply; the "OS actually decides to run
   it in the background, on schedule, under real power/network conditions"
   half realistically still needs real devices. Feasibility: medium --
   partial automation is achievable now, full-fidelity acceptance testing
   is not.
3. **`journey_5_notification_sync` (client notification-event integration).**
   Lowest cost of the three, but blocked on unfinished native-client work
   rather than infrastructure alone -- its own ignore reason says the
   client integration itself is "not production-complete," not that a test
   environment is missing. Whether this needs real devices or could run on
   emulators/simulators depends on a fact not yet established from this
   codebase alone: whether "notification" here means an in-app event
   delivered over this system's own sync transport (fully emulator-testable
   -- no OS push infrastructure involved) or an OS-level push notification
   (would need real or virtual APNs/FCM credentials and device-token
   plumbing, meaningfully more setup). This should be the first thing a
   dedicated follow-up task confirms, since it changes the feasibility
   assessment substantially.

None of this is committed to or built here -- per the task, this is scoping
for a future, dedicated task, and the three journeys remain `#[ignore]`d.

### Verification

| Check | Result |
|---|---|
| `cargo check --workspace` | clean (no Rust source changed this task -- Dockerfiles, shell scripts, and workflow YAML only) |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `cargo test -p api-server --release` | all passing, no regressions |
| `cargo test --workspace --release --no-fail-fast` (real D-Bus/gnome-keyring session, and for the first time in this session a real local Postgres instead of none) | 571 passed, 7 failed -- the 7 are the same disclosed `crates/team8-e2e-tests` testcontainers/Docker-dependent journeys as every prior task (no Docker daemon in this sandbox); having a real local Postgres available this time genuinely resolved all 12 of the previously-disclosed `persistence-postgres` failures, confirming those were exactly the infra gap they were always described as, not a masked defect |
| `python3 scripts/verify/verify_team8_static.py` | 363/363 |
| Deliberately-stale `Cargo.lock` test (see above) | genuinely fails `cargo metadata --locked`; genuinely passes once reverted |
| Deliberately-stale `.sqlx/` test (see above) | genuinely fails `cargo sqlx prepare --check`; genuinely passes once restored |
| `flutter analyze` (real Flutter 3.47.2, installed fresh this session) | 0 issues |
| `flutter test` | 16 passed, 1 skipped (device-lab journey, see H6 Part 2) |
| `bash scripts/verify/verify_mobile.sh` | passes |
| Fresh `workflow_dispatch` of `ci.yml` | see the job table in this task's chat report |

### All six hardening tracks are now complete

H1 (production bootstrap), H2 (distributed session revocation), H3 (relay
topology isolation), H4(a) (CORS), H4(b) (transport security), H5
(deterministic release builds), and H6 Part 1 (CI immutability) are all
landed and CI-confirmed. H6 Part 2 (native acceptance gates) is deliberately
scoped, not built, per the original plan and this task's own instructions --
its assessment above is the explicit next step for a future, dedicated task.
This closes out the production-readiness audit as originally scoped.

## Hardening H7 (relay ticket self-identity binding)

A second-pass, independent re-audit of the H1-H6 work above found a
genuinely new P1 in the very feature H4(b) built to close the previous
relay-token problem. This is that fix.

### The gap H4(b) left open

`POST /api/relay-ticket` (`crates/bins/api-server/src/routes/relay.rs`,
`issue_ticket`) mints a real, well-designed ticket -- 30s TTL, single-use
via `jti`, correctly scoped to a specific `target_id`. But
`IssueTicketRequest` only ever carried `target_id`; the minted
`TokenClaims` never said anything about which *replica* the caller was
entitled to register as once it opened the WebSocket. Separately,
`RelayAuth` (the WS-upgrade query params) has its own `self_replica`
field -- a plain, unauthenticated URL query parameter, parsed at
`relay_route` and handed straight into `RelayRegistry::register()` with
zero verification against the ticket's own claims. The ticket proved who
the authenticated user was and which target they could reach; it proved
nothing about which replica identity they were allowed to claim as their
own on connection.

**Practical consequence, preserved here in the re-auditor's own precise
framing because it is the accurate severity, not a rounder-sounding one:**
a compromised or malicious authenticated client *inside* an organization
could connect while asserting another legitimate replica's UUID as
`self_replica`. Because `register()` replaces whatever was previously
registered under that UUID, this was a **same-tenant
replica-impersonation / connection-displacement / denial-of-service
vector -- not cross-tenant** (the existing organization check on every
frame still held). The relay's existing check that a frame's
`sender_replica` matches the connection's declared `self_id` only proved
internal consistency with the attacker's own unverified declaration, not
actual ownership of that declaration.

### Resolving the `device_id`-vs-`self_replica` question first

The task's own instructions asked to check, before building anything,
whether the relay's replica identity is meant to be the *same* concept as
the already-authenticated actor's `device_id` (`platform-kernel::authority`'s
`ActorContext`) -- if so, the smaller fix is to derive `self_replica` from
that already-verified identity instead of adding a new table.

Read with real evidence, not assumed either way:

- `platform-kernel::versioning::ReplicaId` is a genuinely distinct Rust
  type from `DeviceId` (itself just `pub type DeviceId = ObjectId`) --
  not a literal alias.
- But `desktop-shell/src/lib.rs`'s `SessionInfo::from_session` derives the
  app's own "device_id" (the value returned to the frontend) *directly*
  from `local_replica: ReplicaId`, and `login`'s own doc comment says the
  device's `ReplicaId` is "a property of this physical device/install, not
  of who happens to be logged in" -- strong evidence the two concepts
  really are meant to be the same identity in this codebase's design.
- However, reading the actual wire request both real clients send at
  login settles the practical question: `desktop-shell/src/session.rs`'s
  `LoginRequest { username, password, client_type }` and
  `mobile/lib/net/auth.dart`'s equivalent body carry **no device_id or
  replica_id field at all**. Neither does `TokenClaims`/`AuthenticatedUser`
  for ordinary access/refresh tokens.

So the "preferred, smaller" path is not available today without also
extending the login contract across all three real clients and the JWT
schema for every token type -- a materially larger, riskier change than
this task should make. **Decision: implement Option A**, a durable
ownership table, checked at ticket-issuance time. Option B (per-replica
cryptographic keypairs) is the stronger long-term answer, correctly
identified as such by the re-auditor, and is explicitly deferred future
work -- the same treatment H3 gave the deferred Redis/NATS shared-bus
option, not silently dropped.

### The fix

- **`migrations/{postgres,sqlite}/20260110000000_add_replica_ownership.{up,down}.sql`** --
  a new `replica_ownership` table: `replica_id` primary key, `user_id`,
  `organization_id`, `claimed_at`. First-claim-wins, enforced by the
  primary key itself: `INSERT ... ON CONFLICT (replica_id) DO NOTHING`
  followed by a `SELECT` of the actual owner relies on the database to
  resolve two concurrent first claims atomically, with no
  `SELECT`-then-`INSERT` race window (`claim_replica_ownership` in
  `relay.rs`).
- **`IssueTicketRequest`** gained a required `self_replica` field. The
  caller now declares, at mint time, which replica identity it intends to
  register as.
- **`issue_ticket`** calls `claim_replica_ownership` before minting
  anything: if `self_replica` is unclaimed, it is claimed for the calling
  `user_id` now; if it is already claimed by that same user, minting
  proceeds as before; if it is claimed by a *different* user, minting is
  refused (401) and the attempt is logged.
- **`TokenScope`** (`routes/mod.rs`) gained an optional
  `#[serde(default)] self_replica: Option<String>` field, `None` for
  every existing access/refresh token, `Some(...)` only for a
  `relay_ticket`-typed token whose `self_replica` has passed the ownership
  check above. This reuses `validate_token`'s existing generic
  exp/revocation logic unmodified -- no second claims type was needed.
- **`relay_route`** now checks `claims.scope.self_replica` against the
  WebSocket's `self` query parameter and rejects a mismatch *before* the
  upgrade is accepted, in addition to the pre-existing `target_id` scope
  check. This is the actual fix: the query parameter is still
  unauthenticated on its own, but it can no longer diverge from what the
  ownership-checked ticket actually authorized.
- **`desktop-shell/src/relay_socket.rs`**'s `mint_relay_ticket` now sends
  `self_replica` (the factory's own `local_replica`, which it already held)
  in the ticket-mint request body -- the one real client in this codebase
  that calls this endpoint.
- The "Ticket design" doc comment on `issue_ticket`, and `RelayAuth`'s
  doc comment on its `self` field, were both updated to describe this
  binding -- their previous silence on self-identity verification is part
  of why the original re-audit caught this and the first one didn't.

### Known, disclosed limitation -- not a reopening of the vulnerability

The ownership model is permanent-per-user by design (first-claim-wins).
`desktop-shell`'s own doc comment says a device's `ReplicaId` is stable
per physical install and is deliberately *not* reset on login, so that a
second login after logout doesn't fragment sync history already on that
machine -- which also means a *different* real employee logging into the
same shared physical install would legitimately reuse that same
persisted `ReplicaId`. Under this fix, if a first employee has already
claimed that id under their own `user_id`, a second employee sharing the
device will be refused a relay ticket for it.

This is an availability inconvenience in an intentionally narrow scenario
(a shared desktop install used by more than one person), not a security
hole: refusing to mint the ticket is the fail-safe direction, and it does
not let anyone connect as a replica they don't own. The real fix for this
edge case is the same one that would also obsolete this whole table --
extending the login contract to carry a verified device identity (the
"preferred" path examined and set aside above), or Option B's per-replica
keypairs. Both are deferred, not built here, and both are noted so this
tradeoff isn't rediscovered as a surprise later.

### Verification

Two new tests in `crates/bins/api-server/tests/relay_switchboard.rs`,
against a real bound server and real WebSocket connections (same harness
as every other test in that file, not mocked):

- `relay_rejects_a_connection_that_declares_a_different_replica_than_the_ticket` --
  mints a ticket honestly bound to one replica id, then attempts to open
  the WebSocket declaring a *different*, un-owned replica id via `self`;
  confirms the connection is now genuinely refused (the HTTP upgrade
  itself fails), not just differently formatted.
- `relay_ticket_issuance_refuses_a_replica_owned_by_another_user` -- the
  seeded admin claims a replica id via a real minted ticket; a second,
  genuinely distinct authenticated user (created via the real
  `POST /api/admin/users` route and logged in for a real access token)
  then attempts to mint a ticket declaring that same replica id as their
  own `self_replica`; confirms the mint itself is refused before a ticket
  ever exists.

Both pre-existing legitimate-path assertions in the same file
(`relay_forwards_a_frame_between_two_replicas`, and every `open()` call
throughout the suite, which always mints and connects with the *same*
replica id for the same authenticated user) continue to pass unchanged,
proving the ordinary case still works exactly as before.

| Check | Result |
|---|---|
| `cargo check --workspace` | clean |
| `cargo test -p api-server --test relay_switchboard` | 7 passed (5 pre-existing H4(b) tests unchanged, 2 new H7 tests), 0 failed |
| `cargo test --workspace --exclude desktop-shell` | all passing except the same 7 disclosed, Docker-dependent `crates/team8-e2e-tests` journeys as every prior task in this session (no Docker daemon in this sandbox) -- no new regressions |
| `cargo clippy --workspace --exclude desktop-shell --all-targets -- -D warnings` | clean |
| `cargo clippy -p desktop-shell --all-targets -- -D warnings` | clean (run separately from the rest of the workspace purely to manage this sandbox's limited disk space during Tauri/WebKit's large build; not a narrower check) |
| Existing single-use (`redeem_ticket_once`), target-scoping (`object_id`), and 30s-TTL properties from H4(b) | all three's original tests (`relay_ticket_cannot_be_redeemed_twice`, `relay_ticket_cannot_be_used_against_a_different_target`, and `exp` via `validate_token`) still pass unmodified -- this task added the missing binding without weakening anything already correctly built |
| Fresh `workflow_dispatch` of `ci.yml` | see the job table in this task's chat report |

### Not acted on in this task (flagged by the same re-audit, lower priority)

Two other findings the re-audit surfaced are deliberately out of scope
here and noted so they aren't lost:

- `load-smoke`'s real run-to-run variance at the identical commit
  (0.60%/1.54s p95 vs. 0%/191ms), and the fact it runs
  `DATABASE_URL=sqlite://...` despite spinning up a Postgres service
  alongside it -- meaning it never actually exercises the production
  Postgres path. A real, separate test-design gap.
- Documentation drift: the README's "27 crates"/"Rust 1.75" claims (the
  real count is 41 workspace members, the real toolchain is 1.97.1 per
  `rust-toolchain.toml`), and other docs still describing the pre-H4(a)
  `allow_origin(Any)` CORS behavior and the pre-H5 lockfile-regeneration
  pattern as if current.

## Hardening H10 (mobile observer client-capability enforcement)

Two governance documents (`ONYX-MOB-00_Mobile_Client_Strategy_Manifesto_v1.1`,
`ONYX-MOB-01_Android_Kotlin_iOS_PWA_Technical_Blueprint_v1.1`, both now
under `docs/governance/`) specify a closed `client_type` contract and a
server-enforced `mobile_observer` capability ceiling
(`effective_permissions(user, session) = user_permissions(user) ∩
observer_capabilities(session.client_type)`) in normative, present-tense
"MUST" language. Read against the actual codebase before writing anything,
neither existed: `LoginRequest::client_type` (`routes/auth.rs`) was a
loose `Option<String>`, and the only place it was ever read was one
hardcoded comparison, `payload.client_type.as_deref() == Some("mobile")`,
gating a single mobile-class-access check. No code path anywhere denied a
mutation on the basis of *what kind of client* sent it, and no
unrecognized `client_type` value was ever rejected. This task built the
enforcement the documents describe, for the first time.

### What was built

`crates/bins/api-server/src/routes/client_type.rs`, new:

- **`ClientType`** — a closed, `#[serde(rename_all = "snake_case")]` enum
  (`Mobile`, `MobileObserver`, `Desktop`, `Admin`, `Web`). A plain
  string-valued `Deserialize` enum already rejects any string outside
  this set (`serde::de::Error::unknown_variant`, confirmed against
  current serde docs, not assumed) — this alone satisfies "the backend
  MUST reject unknown client types" with no hand-written validation.
  Confirmed by direct inspection that every real client's login call site
  already sends one of these five literal strings
  (`mobile/lib/net/auth.dart`: `"mobile"`; `desktop-shell/src/session.rs`:
  `"desktop"`; `admin-shell/ui/src/pages/Login.tsx`: `"admin"`;
  `web-ui/src/hooks/useAuth.ts`: `"web"`) — no client-side change was
  needed for this to be a pure tightening, not a breaking change.
- **`ClientType::default_on_absence` → `Web`** — an *absent* `client_type`
  is a distinct case from an unrecognized one. Grepping every real
  internal caller (`crates/bins/api-server/tests/*.rs`, this project's
  end-to-end/integration suites) found dozens of existing tests,
  including the shared `test_harness.rs` used by every end-to-end
  journey, that call `/api/auth/login` without ever sending `client_type`
  at all. Requiring the field outright would have broken those real
  callers, not a hypothetical one. `Web` (full capabilities) was chosen
  as the fallback over inventing a sixth "unclassified" variant, because
  it is the literal continuation of the pre-existing "absent client_type
  is never gated" behavior, not a new policy, and keeps the enum matching
  the five real client classes the governance documents define.
- **`ClientCapabilities`** and **`capabilities_for`** — a static mapping
  (`FULL_CAPABILITIES` for every class except `MobileObserver`;
  `OBSERVER_CAPABILITIES` — every `can_read_*`/`can_download_files` true,
  every mutation flag false — for it), matching ONYX-MOB-01 §8's field
  list exactly and using the "enum/bitset/typed policy object" latitude
  that section explicitly leaves open.
- **`require_capability`** — denies with `403
  CLIENT_CAPABILITY_DENIED` (this project's real `ApiError`/
  `safe_details` envelope, not ONYX-MOB-01 §9's illustrative flat JSON
  example verbatim, per that section's own "must align with ONYX error
  conventions" caveat) unless the session's mapped capability permits the
  action. Wired into every mutation-class endpoint ONYX-MOB-01 §9
  enumerates: mission/task command endpoints, approval decisions,
  lifecycle transitions, conflict resolution, file upload, and
  organization/user/policy/administrative mutation
  (`routes/{admin,auth,command,policy_admin,profiles/*,todo_admin}.rs`).
  This check runs in addition to, never instead of, this project's
  existing per-route authority checks (`require_admin`, ownership checks,
  etc.) — a user who already fails their existing authority check is
  still denied by that check first; the capability ceiling only ever
  narrows further.

### Deliberately out of scope

This project has no pre-existing unified `user_permissions` object to
literally intersect against — authority today is checked ad hoc per route
(`require_admin`, verifier resolution, H2's revocation watermark, H7's
relay ownership, etc.). Retroactively unifying all of that into one real
permissions type, so that `effective_permissions` could be computed as a
literal set intersection rather than "the mutation-class check runs after
the route's own authority check," is a materially larger and riskier
change than this task's scope (closing the `mobile_observer` boundary)
calls for, and was not attempted. `can_read_evidence`/`can_download_files`
are likewise flat bools, not a policy-object hook, per ONYX-MOB-01 §8's
own "policy-controlled" caveat — no real per-file/per-evidence
authorization policy engine exists in this codebase to hook into today.

### Governance document corrections

Both `ONYX-MOB-00` §4 and `ONYX-MOB-01` §26 P1 read, on a literal
reading, as if this enforcement already existed and only needed
documenting. Both now carry an explicit "Implementation note (H10)"
pointing at this entry and at `client_type.rs`, and `ONYX-MOB-01`'s single
P1 bullet list has been expanded into P1.1–P1.5, each naming the actual
file/module/test that satisfies it, so a future reader cannot mistake the
blueprint's aspirational phasing for a record of prior work.

### Verification

New test file
`crates/bins/api-server/tests/mobile_observer_capability.rs`, against a
real bound server and real authenticated sessions (same harness pattern
as every other integration test in this crate):

- `mobile_observer_reads_normally_but_every_mutation_endpoint_denies_it`
  -- a session that declares `client_type = "mobile_observer"` at login
  continues to succeed on read endpoints while every representative
  mutation endpoint now returns `403 CLIENT_CAPABILITY_DENIED`.
- `cross_tenant_command_still_rejected_independent_of_client_capability`
  -- confirms the new capability check is additive: an existing,
  unrelated authority check (cross-tenant access) still fires on its own
  terms regardless of client class.
- `observer_session_refresh_preserves_the_capability_ceiling` -- refreshing
  a `mobile_observer` session's token does not silently reset it to full
  capabilities.

| Check | Result |
|---|---|
| `cargo check --workspace` | clean |
| `cargo test -p api-server` (full crate, all test files) | 46 passed, 0 failed |
| `cargo clippy -p api-server --all-targets -- -D warnings` | clean |
| `cargo clippy --workspace --exclude desktop-shell --exclude admin-shell --all-targets -- -D warnings` | clean |
| `cargo test --workspace --exclude desktop-shell --exclude admin-shell` | all passing except the same 7 disclosed, Docker-dependent `crates/team8-e2e-tests` journeys as every prior task in this session (no Docker daemon in this sandbox) -- no new regressions |
| `cargo fmt --all -- --check` | clean |

## H10.M0 (freeze the Flutter Android reference implementation)

Migration Sequence step 1 (ONYX-MOB-00 §25) / Android Work Package A0
(ONYX-MOB-01 §25), sequenced immediately after H10 per both governance
documents' agreed order. This is a documentation-and-process task, not
development: per ONYX-MOB-00 §8, the Flutter client becomes a Frozen
Reference Implementation ("no ordinary new product development;
security fixes MAY continue; critical defects MAY continue") while a
native Kotlin Android rewrite and an iOS Observer PWA are built
separately. No `mobile/lib/` application behavior was changed by this
task -- the point is capturing exactly what exists today as the ground
truth those rewrites must match, and making the freeze a real,
enforced invariant rather than a written-only policy.

### `docs/mobile-migration/parity-matrix.md` (new)

Per ONYX-MOB-01 §5's repository layout, confirmed not to already exist.
Documents, screen by screen (Dashboard, Missions list, Mission Detail,
Tasks list, Task Detail, Approvals, Notifications, Files, Settings,
both login screens, startup/error recovery, the shared-refresh
controller architecture, and the full FFI contract surface): what each
does today, which real backend endpoints/`mobile-core` FFI functions it
calls, what state it reads/writes, and non-obvious behavior. This is
written as real acceptance criteria for the Kotlin rewrite, not a
high-level summary -- e.g. it pins down that Approvals is a filtered
view over already-loaded Task/Mission state (not its own aggregate,
and `controller.approvals` is loaded but never actually populated or
read by any screen), that Mission's decision commands are
`ActivateMission`/`RejectApproval` (not a direct `ApproveMission`
mirror of Task's `ApproveTask`/`RejectTask` shape), the exact
reason-required-before-Reject gating, and the literal `"mobile"`
`client_type` value sent at login (`mobile/lib/net/auth.dart:46`).

**A real discrepancy surfaced and resolved while building this
document**, worth recording since it corrects this task's own starting
assumption: the FFI contract is 18 functions declared in
`mobile-core.h`, not "17 plus one Android-specific" as this task's own
instructions assumed. Reading the header and every real call site
(Dart's `lib/bridge/*.dart`, Kotlin's `WorkManagerService.kt`, Swift's
`BackgroundService.swift`) directly: 15 functions are called from
Dart, `mobile_core_android_do_work` is called from Kotlin (not Dart --
an `external fun nativeAndroidDoWork()` bound via
`System.loadLibrary`), `mobile_core_background_sync_registered` is
called from Swift (via `dlsym`, not a static import), and
`mobile_core_ios_background_sync` was not found called from any file
this review reached in either Dart, Kotlin, or the one Swift file
read -- left as a real, disclosed open question (possibly dead code,
possibly called from a Swift file not read in this task) rather than
silently assumed resolved.

### Real, current test baseline (re-run fresh, not assumed from a prior session)

Against this task's real tip (this branch, carrying H10):

```
flutter analyze  ->  No issues found! (ran in 16.7s)
flutter test     ->  16 passed, 1 skipped, 0 failed
```

The one skip (`test/integration/p2p_sync_test.dart`) is real and
disclosed, not silently dropped: `Skip: Requires two authorized
iOS/Android devices and ONYX_MOBILE_DEVICE_TEST=1` -- gated behind an
explicit opt-in environment variable because it genuinely cannot run
without two real physical/authorized devices. This 16-passed/1-skipped/
0-failed baseline, recorded per-test in the parity matrix's final
section, is the parity floor the Kotlin rewrite's own (differently
structured, not line-for-line ported) test suite must not regress
below.

### Freeze enforcement: a real CI gate, not a verbal policy

This project has consistently preferred enforced invariants over
written-only agreements (H1's production-mode bootstrap refusal, H10's
`ClientType` rejection). Two options were weighed for making "no
ordinary new product development in `mobile/`" real: a hard CI gate
requiring an explicit override marker per change, versus a lighter
`CODEOWNERS`/README-notice-only approach. The hard CI gate was chosen
-- this project's own prior pattern (a *rejected* invalid state, not
just a documented one) is the closer fit than a purely social
convention, and the gate's cost is low: it only fires on diffs that
actually touch `mobile/lib/`, which per this task's own scope should
now be rare.

**`scripts/verify/verify_mobile_freeze.sh`** (new), wired as the
`mobile-freeze-guard` job in `ci.yml` (runs on every push/PR, ahead of
`mobile-dart`): computes `git diff --name-only` between the merge-base
of `origin/main` and `HEAD`; if any changed path starts with
`mobile/lib/`, the diff must also touch `mobile/FROZEN_EXCEPTION.md`
(new, a real exception log with instructions and an empty log section)
or the job fails with a message explaining exactly why and what to do.
Deliberately scoped to `mobile/lib/` only -- not `mobile/test/`,
`mobile/android/`, `mobile/ios/`, or `mobile/tool/` -- since platform
scaffold, CI, and test maintenance needed to keep the frozen app
building on newer toolchains is not "new product development" and
gating it would make the freeze actively harmful rather than useful.

`mobile/README.md` also gained a prominent freeze notice pointing at
both the parity matrix and the exception file, per the task's
proportionality question -- both the process guard and the visible
documentation were built, not one instead of the other, since the
guard alone is invisible until someone's diff already fails it.

### Verification

Real test-then-revert proof the guard actually works, run against this
task's own M0 commit as the base (not a hypothetical):

1. Appended a trivial comment to `mobile/lib/main.dart`, committed
   without touching `FROZEN_EXCEPTION.md`.
   `verify_mobile_freeze.sh <M0-commit>` -> **exit 1, BLOCKED**, real
   error message printed.
2. Amended that commit to also touch `mobile/FROZEN_EXCEPTION.md`.
   `verify_mobile_freeze.sh <M0-commit>` -> **exit 0, OK**.
3. `git reset --hard` back to the real M0 commit -- the test commit
   never reached the pushed branch.

| Check | Result |
|---|---|
| `flutter analyze` (mobile/) | clean, 0 issues |
| `flutter test` (mobile/) | 16 passed, 1 disclosed skip, 0 failed |
| `verify_mobile_freeze.sh` block case | confirmed blocks (exit 1) |
| `verify_mobile_freeze.sh` exception case | confirmed passes (exit 0) |
| Fresh `workflow_dispatch` of `ci.yml` | see the job table in this task's chat report |

### Not built in this task (explicitly out of scope per the task's own instructions)

`mobile-android/` (the Kotlin project) and `mobile-pwa/` were not
touched or started -- those are A1 and P2 respectively, later,
separate tasks. No `mobile/lib/` application code or behavior was
changed.
