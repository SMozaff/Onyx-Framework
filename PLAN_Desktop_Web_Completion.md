# ONYX — Desktop & Web Completion Plan

**Status:** Draft for review — not yet executed
**Author:** Planning pass by Claude, session dated 2026-08-11
**Source of truth used:** `ONYX_Unified_Technical_Blueprint_v1_6.docx` (uploaded, per
user instruction this session), `DECISIONS.md` and related decision logs,
`Team_Prompt_5_-_Desktop___Mobile_Native_Clients.md`, `Team_Prompt_6_-_Web_UI_Thin_Client.md`,
and direct inspection of the delivered codebase (`Onyx-Framwork-main`).

This document exists per the user's standing instruction: any code change or
scope decision made in this project must be documented and registered in a
markdown file. Nothing in this plan has been executed yet — it is a proposal
for review before implementation begins.

---

## 1. Confirmed roadmap (as instructed this session)

| Phase | Scope | Timing |
|---|---|---|
| **Phase 1** | Desktop + Web brought to full, published completeness | **Now — current focus** |
| **Phase 2** | Add text messaging to the Web platform | **After Phase 1 ships** |
| **Phase 3** | Finish the Flutter mobile app | **After Phase 2, on explicit go-ahead** |

Additional standing constraints from this session:

- **BLE and Wi-Fi Direct code stay in the codebase, untouched**, for now. Not
  removed, not modified. (Superseded an earlier instruction to remove BLE
  entirely — the user chose to keep both after seeing they are frozen,
  tested contracts per Team Prompt 4 §3.3/§7.2.)
- **Web UI will *not* get file sharing**, ever, per explicit instruction —
  this is a permanent exclusion, not a deferral.
- **Web UI *will* get messaging**, but only in Phase 2, after Phase 1 ships.
- **Mobile/Flutter work is paused** until the user explicitly says to resume.
- UX/visual design is **explicitly not final** — expect churn. Phase 1 targets
  functional completeness, not finished visual design. Phase 2's messaging
  UI has a stated *style direction* (Telegram-like, friendly/playful) but
  this is a later-phase concern, not something to lock in now.
- File chunk size / max file size should become **admin/manager configurable**
  rather than hardcoded — flagged in §4 as a real domain-model change, not
  a settings tweak, because it currently lives inside a frozen aggregate
  invariant.

---

## 2. What's actually true in the codebase today (verified, not assumed)

This section exists because several assumptions turned out to be wrong
during investigation this session. Everything below was confirmed by
reading source, not inferred from the status report or blueprint alone.

### 2.1 Desktop (`crates/bins/desktop-shell`)

- **Backend/command layer is already complete** for Mission, Task,
  Conversation, Message, FileAsset, and UploadSession. `client-composition`'s
  `AppState::new()` (`crates/applications/client-composition/src/app_state.rs`)
  registers creation and decision handlers for all six aggregate types,
  backed by real SQLite repositories. Desktop-shell shares this composition
  root verbatim — no per-domain wiring lives in desktop-shell itself, by
  design (Team Prompt 5 §4.1, ruling C1).
- **Desktop's Tauri command surface is domain-agnostic**: `execute_command`
  and `execute_query` in `crates/bins/desktop-shell/src/lib.rs` accept a
  generic `CommandEnvelope`/`QueryEnvelope` and dispatch through the shared
  registries. No new Tauri commands are needed to support Messaging or File
  sharing at the transport level.
- **The actual gap is the frontend only.** `crates/bins/desktop-shell/ui/src/pages/`
  currently has: `Dashboard.tsx`, `Approvals.tsx`, `Missions.tsx`, `Tasks.tsx`.
  There is no Messaging page and no File-sharing page anywhere in the
  desktop UI.
- `IdempotencyStore` is in-memory only (flagged pre-existing gap, not
  specific to this plan — see `DECISIONS.md` ruling F1).

### 2.2 Web (`web-ui` + `crates/bins/api-server`)

- Web-ui pages today: `Dashboard`, `Missions`, `Tasks`, `Approvals`, `Reports`,
  `Notifications`, `Login`. No Messaging, no File sharing.
- **`api-server`'s command route is far more limited than desktop's.**
  `crates/bins/api-server/src/routes/command.rs` only supports two command
  types: `notification.Acknowledge` and `approval.Approve`/`approval.Reject`.
  Mission and Task have **no write path on web at all** — Mission/Task
  appear to be read-only on web today (via the query route), a detail not
  previously surfaced in the status report.
- `ApiState` (`crates/bins/api-server/src/routes/mod.rs`) only holds
  `notification_repo` and `approval_repo`. There is no `conversation_repo`,
  `message_repo`, `mission_repo`, or `task_repo` on the web-server side —
  Mission/Task reads go through a separate projection-pool query handler
  (`query_handler.rs`), not the same `Repository` abstraction desktop uses.
- `query_handler.rs` supports: `mission.list/detail`, `task.list/detail`,
  `timeline.list`, `notification.list`, `approval.list`, `report.detail`,
  `dashboard.summary`. **No `conversation.*` or `message.*` query type
  exists.**
- **Web's exclusion of Messaging and File sharing is spec-mandated and
  CI-enforced**, not accidental. `Team_Prompt_6_-_Web_UI_Thin_Client.md` §7
  explicitly lists both as "Not in v1." `web-ui/tests/feature-audit/excluded-features.test.ts`
  fails the build if `uploadFile`, `MeetingChat`, `Blueprint` authoring, or
  a few other patterns appear anywhere in `web-ui/src`. This test will need
  a deliberate, documented edit in Phase 2 (remove the messaging-related
  pattern only; file-upload patterns stay excluded permanently).
- Web-ui already has a `/api/events` WebSocket
  (`crates/bins/api-server/src/routes/events.rs`) that forwards command-result
  events to connected clients. Whether this is suffient for real-time message
  delivery, or needs its own event path, is an open question for Phase 2
  (§5).

### 2.3 File size / chunk size configurability

- `MAX_FILE_SIZE_BYTES = 100 * 1024 * 1024` is a hardcoded Rust `const` in
  `crates/domains/file-domain/src/value.rs`, documented there as a **frozen
  architecture decision** ("100 MB max per each file upload attempt by any
  user"). It's enforced directly inside `FileAsset`'s/`UploadSession`'s
  `decide()` logic in `crates/domains/file-domain/src/aggregate.rs` (three
  separate enforcement points: declared total size, per-chunk running
  total, and version creation).
- No settings/policy/quota table exists anywhere in the schema today
  (`migrations/postgres/20260101000000_initial_schema.up.sql` was checked
  directly — no matches).
- Making this admin/manager-configurable is therefore **new domain and
  infrastructure work**, not a config flag: it means introducing an
  organization-scoped policy value that the aggregate reads instead of a
  compile-time constant, plus a place to store and manage that value, plus
  UI for an admin/manager to change it. See §4.3.

---

## 3. Phase 1 — Desktop & Web completion (current focus)

### 3.1 Desktop: Messaging UI (new)

**Goal (per user instruction): the fullest, most complete version** — not a
stripped-down v1. Build against the full backend capability that already
exists:

- Conversation list view (create, view members, archive)
- `AddMember` to an existing conversation
- Thread/message view: post, edit, redact (soft-delete with tombstone,
  per the domain model), add/remove reactions
- Real-time updates via the existing `subscribe_events` Tauri command /
  `onyx:event` emission (already wired generically; needs a frontend
  listener scoped to conversation/message event types)

**New desktop-shell UI work:**
- `ui/src/pages/Messaging/` — conversation list + thread view
- `ui/src/hooks/` — a messaging-specific data hook alongside the existing
  `useQuery`/`useCommand`
- Routing entry in `App.tsx` / `MainLayout.tsx`

**No backend changes required** for this piece — confirmed in §2.1.

### 3.2 Desktop: File-sharing UI (new)

- File list / browser view scoped to a Mission or Task context (per the
  File domain's cross-domain relationships in the blueprint §4.8.6)
- Upload flow: native file picker (Tauri's file-dialog plugin) → chunked
  upload against `StartUpload`/`AppendChunk`/`FinalizeUpload`
- Version history view (`CreateVersion`)
- Access control UI: `GrantFileAccess`/`RevokeFileAccess`
- Quarantine/archive states surfaced read-only (admin-triggered actions
  like `QuarantineFile`/`ArchiveFile` — need confirmation in §6 on whether
  these are user-facing or ops-only)

**Depends on §4.3** (configurable chunk size) if that lands first —
otherwise ships against the current hardcoded 100MB constant and is
updated once the policy mechanism exists.

### 3.3 Web: no new domain features this phase

Per instruction, Web stays at its current spec'd scope for Phase 1
(Mission/Task read-only, Approvals, Reports, Notifications, Dashboard). No
Messaging, no File sharing. This phase's Web work is about hardening and
publish-readiness, not new features:

- Verify/close the Mission/Task **write gap** noted in §2.2 — confirm
  whether Mission/Task actions (approve, activate, etc.) are meant to be
  web-actionable at all in v1, or if web is intentionally read-only for
  those two domains outside of Approvals. **This needs your confirmation
  — see §6, item 1.**
- CI/test/build verification pass (the org's own `STATUS_REPORT.md` flagged
  a CI re-run in progress and an unrelated mobile-iOS failure — neither
  blocks desktop/web, but should be confirmed green before calling this
  phase "published")
- Confirm `IdempotencyStore`'s in-memory nature is acceptable for this
  release or needs a durable adapter before publish (existing flagged gap,
  not new)

### 3.4 Cross-cutting for Phase 1

- Document every new file/change in this plan's changelog (§7) as work
  proceeds, per the user's standing documentation instruction.
- No changes to `selector.rs`, BLE, or Wi-Fi Direct code.
- No changes to Flutter/`mobile/`, `mobile-core`.

---

## 4. Phase 2 — Web Messaging (deferred until after Phase 1 ships)

### 4.1 Backend (`api-server`) additions needed

1. Add `conversation_repo: Arc<dyn Repository>` and
   `message_repo: Arc<dyn Repository>` to `ApiState`.
2. Extend `command.rs`'s match arms to support:
   `conversation.CreateConversation`, `conversation.AddMember`,
   `conversation.ArchiveConversation`, `message.PostMessage`,
   `message.EditMessage`, `message.RedactMessage`, `message.AddReaction`,
   `message.RemoveReaction` — following the existing Notification/Approval
   pattern (each command type gets its own `AggregateRoot` wiring or reuses
   the domain crate's existing `Conversation`/`Message` aggregates directly,
   to be decided at implementation time; needs a design pass since
   Notification/Approval currently define their aggregates *inline in the
   route file*, which will not scale cleanly to the full Communication
   domain — likely should use `communication-domain`'s real aggregate types
   the way `client-composition` already does).
3. Extend `query_handler.rs` with `conversation.list/detail` and
   `message.list/detail` (or equivalent) query types.
4. **Real-time delivery — open question, see §6 item 2.** Either:
   - (a) reuse `/api/events`, filtering/extending it to carry message
     events, or
   - (b) add a dedicated messaging WebSocket channel.

### 4.2 Frontend (`web-ui`) additions needed

- `src/pages/Messaging/` — conversation list + thread view
- `src/api/` — command/query calls for the new types, following the
  existing `command.ts`/`query.ts`/`events.ts` pattern
- **Visual direction: Telegram-like, friendly/playful tone**, per user
  instruction — to be designed at implementation time, not now, since UX
  is expected to change. This plan intentionally does not lock in specific
  visual details.

### 4.3 Governance change required

- Update `web-ui/tests/feature-audit/excluded-features.test.ts` (and the
  matching pattern documented in `Team_Prompt_6_-_Web_UI_Thin_Client.md` §6.3's
  test snippet) to remove the `MeetingChat`-style exclusion, while
  explicitly keeping `FileUpload`/`uploadFile` in the excluded list. This is
  a deliberate, partial supersession of Team Prompt 6 §7 — should be logged
  as a new ruling in `DECISIONS.md` when implemented, per the project's own
  documentation convention.

### 4.4 Configurable file chunk size / cap (applies to both Desktop and any
future Web file work, tracked here since it's Communication-adjacent
infrastructure)

This is domain-model work, not a settings toggle:
- Introduce an organization-scoped policy/quota concept (none exists yet —
  confirmed in §2.3). Natural home per the blueprint is the **Policy**
  bounded context (§4.18, "Quota & Rate Governance" §4.18.10 already exists
  as a concept there) — but Policy is one of the 14 bounded contexts from
  the blueprint not yet implemented as a crate (see the open question raised
  earlier this session, still unresolved — **flagged again in §6, item 3**).
- Until that's resolved, a narrower interim option: store a single
  org-scoped `max_file_size_bytes` value (with the current 100MB as
  default) in a new small table/adapter, read by `file-domain`'s aggregate
  via a port instead of the hardcoded constant, with an admin/manager-only
  command to change it. This is a smaller, scoped alternative to standing
  up the full Policy context — but changes a documented frozen invariant,
  so it needs to be logged as a formal ruling in `DECISIONS.md`, not made
  silently.

---

## 5. Phase 3 — Mobile / Flutter (paused, resumes on explicit instruction)

No planning detail is produced for this phase yet. Per instruction, this
resumes only when the user says so. When resumed, it inherits:
- BLE and Wi-Fi Direct code as currently implemented (untouched during
  Phases 1–2)
- Whatever Messaging/File UI patterns and API shapes were established in
  Phases 1–2, for consistency across clients where reasonable

---

## 6. Open questions (need your answer before implementation starts)

1. **Web Mission/Task write gap (§2.2, §3.3):** `api-server` currently has
   no command path for Mission or Task at all — only Notification and
   Approval. Is Web meant to be read-only for Mission/Task in this
   delivery (matching what's built), or is this an undiscovered gap that
   also needs closing in Phase 1? This wasn't part of your instruction, but
   it directly affects whether Phase 1 "publish-ready" is actually
   achievable as scoped, since Mission/Task are the two primary domains,
   and I don't want to declare the project complete while silently leaving
   this gap in place.

2. **Phase 2 real-time delivery mechanism (§4.1.4):** should new web
   messages arrive via the existing `/api/events` WebSocket, or a dedicated
   channel? This affects backend design, not just UI, so I'd like to
   confirm before Phase 2 starts (not urgent now, but flagged so it isn't
   forgotten).

3. **Configurable file cap's proper home (§4.4):** should this be built as
   a narrow, scoped policy value (smaller, faster, but a partial/ad-hoc
   implementation of "Policy"), or should it wait for a real Policy bounded
   context to be built out per the blueprint? This is a real architectural
   fork, not a detail — I don't want to guess which one you want.

4. **Desktop admin/ops actions on File (§3.2):** are `QuarantineFile` and
   `ArchiveFile` meant to be user-facing in the desktop UI, or
   ops/admin-only (e.g. a separate admin panel, or not exposed in UI at
   all yet)? Affects whether Phase 1's File UI needs an admin-mode
   surface.

---

## 7. Changelog

*(To be updated as work is actually executed. No implementation has
happened yet as of this document's creation.)*

- **2026-08-11** — Plan created. No code changed yet.
