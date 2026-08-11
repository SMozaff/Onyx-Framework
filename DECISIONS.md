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
