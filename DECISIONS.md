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
