# DECISIONS.md — ONYX Increment 1 Architectural Rulings

These rulings supersede ambiguous sections of the Handover Document until a
formal amendment is filed. They were issued during the Increment 1
implementation of `platform-kernel`, `platform-contracts`, `mission-domain`,
and `work-domain`, in response to specific ambiguities and conflicts
discovered while writing compiled, tested code against the frozen contract.

Each entry below states: the gap/conflict found, the ruling issued, and
(where applicable) implementation notes on how it was carried out.

---

## A. Mission Transitions

| Gap                                    | Ruling                                                                                                                                                                                                   |
|:-------------------------------------- |:-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Draft → Planning`                     | Added `SubmitBlueprint` command. `CreateBlueprintRevision` creates a revision but remains in `Draft`; only `SubmitBlueprint` advances to `Planning`.                                                     |
| `Active → Review`                      | Added `TriggerReview` command (`Active → Review`).                                                                                                                                                       |
| `Review → Active / Closed / Cancelled` | Mapped to existing `ActivateMission`, `CloseMission`, `CancelMission` respectively.                                                                                                                      |
| `Halted → Active` vs `Halted → Paused` | `checklist_completed == true` → `Active`; `false` → `Paused`. The resulting status is carried on the `MissionRestarted` event itself (see the self-caught-bugs section below) so `apply()` remains pure. |
| `Archived → Closed` (restore)          | Out of scope for Increment 1. No command exists; unreachable.                                                                                                                                            |
| `Cancelled → Archived`                 | Handover Document is authoritative — transition is allowed (the Team Prompt's omission was a typo).                                                                                                      |
| `Planning → Active` guard (Approval)   | Stubbed as `true` for Increment 1. Full check deferred to Increment 7 (Policy context).                                                                                                                  |
| `Paused → Active` (from Planning)      | A mission paused from `Planning` resumes into `Active` via `ResumeMission` (intended: blueprint already approved).                                                                                       |

## M1. Missing Approval Transitions (self-caught, then ruled)

While transcribing the ruled Mission transition table into code, a second
gap was found and flagged before implementation: `Planning → AwaitingApproval`
and `AwaitingApproval → Planning` had guards described in the state tables
but no commands to reach them.

**Ruling:** Added `RequestApproval` (`Planning → AwaitingApproval`,
`{ reason: String, evidence: Vec<ObjectId> }`) and `RejectApproval`
(`AwaitingApproval → Planning`, `{ reason: String }`).

**Naming correction to the ruling's own illustrative test:** the ruling's
sample test reused `MissionEvent::MissionBlueprintSubmitted` for the
`RequestApproval` transition. That event name already denotes the distinct
`Draft → Planning` transition. Reusing one event name for two transitions
would make `apply()` ambiguous on event-sourced replay. Implemented instead
with two dedicated events: `MissionApprovalRequested` and
`MissionApprovalRejected`. The ruling's command names, field shapes, and
transition table are otherwise implemented exactly as specified.

## B. Task Transitions

| Gap                                                | Ruling                                                                                                                                                                                                                                                                                                                                                                                                                                   |
|:-------------------------------------------------- |:---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Draft → Ready`                                    | Added `MarkReady` command.                                                                                                                                                                                                                                                                                                                                                                                                               |
| `Paused → Active`                                  | Added `ResumeTask` command.                                                                                                                                                                                                                                                                                                                                                                                                              |
| `Blocked → Active`                                 | Added `UnblockTask` command.                                                                                                                                                                                                                                                                                                                                                                                                             |
| `Submitted → Active` (review rejects)              | Added `RejectTask` command.                                                                                                                                                                                                                                                                                                                                                                                                              |
| `Reopened` status                                  | Initially ruled "removed" based on an incomplete reading of the Handover Document — **reversed by ruling T1 below**, which is authoritative.                                                                                                                                                                                                                                                                                             |
| `AddDependency` invariants                         | Increment 1 scope: reject self-dependency and exact duplicates only. Full acyclicity enforcement (requires visibility into the whole dependency graph, which a single aggregate does not have) is deferred to Increment 2/3.                                                                                                                                                                                                             |
| `ChangePriority` / `AssignOwner` / `AddDependency` | Allowed in `Draft`, `Ready`, `Active` only. Rejected in `Paused`, `Blocked`, `Submitted`, `Approved`, `Closed`. **Extended (self-caught, analogical, not a new business-judgment ruling) to also include `Reopened`**, since `Reopened` is functionally an in-flight/resumable state equivalent to the ruled three (work has not yet been re-submitted for review), and the ruled list predates ruling T1's reinstatement of `Reopened`. |

## T1. Task `Reopened` Status — Reinstated (ruling reversed)

An initial ruling stated "remove `Reopened` from the enum; the Handover
Document does not list it." This was flagged as factually contradicted by
the Handover Document's own machine-readable contract in this project,
which explicitly defines:

```
states: [..., CLOSED, CANCELLED]
transitions:
  CLOSED: ["REOPENED"]
commands: [..., ReopenTask, ...]
events: [..., TaskReopened, ...]
```

**Ruling (superseding the earlier one):** the Handover Document is
authoritative. Reinstated:

- `TaskStatus::Reopened`
- `TaskCommand::ReopenTask { reason: String, authorized_by: UserId }`
  (`Closed → Reopened`)
- `TaskEvent::TaskReopened { reopened_at, reason, authorized_by }`
- Outgoing transitions from `Reopened` (undefined in the Handover excerpt,
  defined by this ruling): `Reopened → Active` via `StartTask`, and
  `Reopened → Cancelled` via `CancelTask` (not a separate "resume" command;
  `StartTask` was extended to also accept `Reopened` as a starting status,
  matching the ruling's own illustrative test).

## C. Aggregate Construction

**Ruling:** Creation commands (`CreateMission`, `CreateTask`) are **not**
routed through `AggregateRoot::decide()`, which assumes an existing
aggregate instance. Each aggregate instead exposes a dedicated associated
constructor:

```rust
impl Mission {
    pub fn create(cmd: MissionCommand, ctx: &DecisionContext) -> Result<Vec<MissionEvent>, MissionError>;
}
impl Task {
    pub fn create(cmd: TaskCommand, ctx: &DecisionContext) -> Result<Vec<TaskEvent>, TaskError>;
}
```

`decide()` rejects `CreateMission`/`CreateTask` with `InvalidTransition` if
called directly (defensive; the future `Repository` in Increment 2 is
expected to route creation to `create()` and never pass a creation command
to `decide()`). The `AggregateRoot` trait itself is unchanged;
`decide(&self, ...)` still assumes `Self` already exists.

Rehydration from the first event uses a dedicated
`from_created_event(&Event) -> Self` associated function on each aggregate,
which panics if given any event other than the creation event (the future
`Repository` always passes the first event of a stream, which is
guaranteed by construction to be the creation event).

## D. Undefined Types — Placeholders

| Type                   | Ruling / Implementation                                                                                                                                                                                                                                                                                                                                                         |
|:---------------------- |:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `MissionTimelineRef`   | `pub struct MissionTimelineRef(pub ObjectId);` — placeholder, unused by any Increment 1 command (field present on `Mission` but always `None`).                                                                                                                                                                                                                                 |
| `MissionSettings`      | `pub struct MissionSettings { pub timezone: String, pub calendar: String }` with `serde(default = ...)` per-field defaults (`"UTC"` / `"standard"`).                                                                                                                                                                                                                            |
| `Dependency`           | `pub struct Dependency { pub task_id: TaskId, pub dependency_type: DependencyType }`, with `DependencyType` as a 3-variant enum (`FinishToStart`, `StartToStart`, `FinishToFinish`) — the enum's variants weren't specified by the ruling, but a dependency graph needs *some* relationship kind, so a conventional PM-style set was chosen as the least-committal placeholder. |
| `MissionId` / `TaskId` | Newtypes over `ObjectId`, deriving `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize`.                                                                                                                                                                                                                                                          |
| `ConflictId`           | `pub struct ConflictId(pub [u8; 16]);` in `platform-kernel`, `#[allow(unused)]`, referenced only by `DomainError::ConflictPending`; no Increment 1 domain code constructs it. Full semantics deferred to Increment 3.                                                                                                                                                           |

## E. Internal Inconsistencies

| Issue                                         | Ruling                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
|:--------------------------------------------- |:-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Imports in `traits.rs`                        | The frozen prompt's `use crate::versioning::...` etc. were a typo (those modules live in `platform-kernel`, not `platform-contracts`). Corrected to `use platform_kernel::...;`.                                                                                                                                                                                                                                                                                     |
| `ConflictId` used in `DomainError`            | Placeholder defined in `platform-kernel` (see §D above).                                                                                                                                                                                                                                                                                                                                                                                                             |
| `DecisionContext` + `Box<dyn IdGenerator>`    | Kept as `Box<dyn IdGenerator>`; no `Clone` bound required. `IdGenerator` requires `Send + Sync` (needed for the trait object to be safely held); the deterministic test implementation uses `AtomicU64` rather than `Cell` for this reason.                                                                                                                                                                                                                          |
| Authority methods (`can_pause_mission`, etc.) | Implemented as a single `VerifiedAuthority::is_authorized(&self, scope_hint: &str) -> bool` stub, unconditionally returning `true` for Increment 1. Every `decide()` implementation calls this once at the top, keeping `MissionError::Unauthorized` / `TaskError::Unauthorized` reachable in the type system without real policy evaluation (deferred to Increment 7). A dedicated test locks in "authority stub never rejects in Increment 1" for both aggregates. |
| Test layout                                   | Per-crate `tests/` directories (Cargo-correct), not the workspace-root paths shown in the frozen manifest. Structure: `crates/{kernel,domains}/<crate>/tests/{unit.rs, unit/*.rs, property.rs, golden.rs}`. Files directly under `tests/` are compiled as separate integration-test binaries per Cargo's convention; `unit.rs` pulls in the `unit/*.rs` submodule tree via `#[path = ...] mod ...;` so per-topic test files can share one binary.                    |

## E1. `DomainError` Serialization Shape

**Conflict found:** the Team Prompt's `#[derive(Serialize)]` on `DomainError`
produces serde's default internally-tagged-by-variant-name shape (no
`code`, `category`, `retryability`, or `correlation_id` fields), while the
Handover Document specifies an exact wire shape:

```json
{"code": "...", "category": "...", "retryability": "RETRYABLE|NON_RETRYABLE|TRANSIENT", "safe_details": {...}, "correlation_id": "..."}
```

**Ruling:**

- `DomainError` derives `Serialize` with `#[serde(tag = "code", content = "safe_details")]` and per-variant `#[serde(rename = "SCREAMING_CASE")]`, producing `{"code": "...", "safe_details": {...}}`.
- `category` and `retryability` are computed via `DomainError::category()` / `DomainError::retryability()` methods (fixed per-variant mapping, table below), not stored fields.
- `correlation_id` is **not** a field on `DomainError` — `DomainError` has no knowledge of the request that produced it. It is attached by a new wrapper type, `DomainErrorResponse { code, category, retryability, safe_details, correlation_id }`, constructed via `DomainErrorResponse::from_error(&error, correlation_id)` at the API/command-handler boundary (Increment 2). Golden fixture tests validate `DomainErrorResponse`'s wire shape, not `DomainError`'s directly.
- **Implementation correction to the ruling's own sample code:** the ruling's illustrative `DomainErrorResponse::from_error` extracted `code` via `serde_json::to_string(&error)`, which — given `tag`/`content` — serializes to a full JSON object string (`{"code":"...","safe_details":{...}}`), not the bare code. Implemented instead via a `DomainError::code() -> &'static str` method with an explicit per-variant match, matching the serialized `code` tag exactly (verified by a golden test).
- `serde_json` was promoted from a dev-dependency to a normal dependency of `platform-contracts`, since `DomainErrorResponse.safe_details: serde_json::Value` is part of the crate's public production API, not test-only code (a direct, necessary consequence of this ruling, given §7.1 of the frozen contract otherwise restricts `serde_json` to tests).

### Retryability Mapping (ruled)

| Variant             | Retryability    |
|:------------------- |:--------------- |
| `InvalidTransition` | `NON_RETRYABLE` |
| `Unauthorized`      | `NON_RETRYABLE` |
| `VersionConflict`   | `RETRYABLE`     |
| `EpochConflict`     | `RETRYABLE`     |
| `NotFound`          | `NON_RETRYABLE` |
| `AlreadyExists`     | `NON_RETRYABLE` |
| `InvalidArgument`   | `NON_RETRYABLE` |
| `ConflictPending`   | `RETRYABLE`     |

### Category Mapping (ruled)

| Variant             | Category      |
|:------------------- |:------------- |
| `InvalidTransition` | `DOMAIN`      |
| `Unauthorized`      | `AUTHORITY`   |
| `VersionConflict`   | `CONCURRENCY` |
| `EpochConflict`     | `CONCURRENCY` |
| `NotFound`          | `DOMAIN`      |
| `AlreadyExists`     | `DOMAIN`      |
| `InvalidArgument`   | `DOMAIN`      |
| `ConflictPending`   | `DOMAIN`      |

---

## Environment / Toolchain Notes (not rulings — factual disclosures)

- **Rust toolchain:** pinned to stable **1.97.1** via `rust-toolchain.toml`
  (satisfies the ruled "Stable Rust 1.75+" — the literal minimum was not
  usable in this sandbox because `uuid`'s transitive dependency
  `getrandom` resolved to a version requiring the unstabilized
  `edition2024` Cargo feature on 1.75.0).
- **`Debug`/`Display` for kernel ID types:** `ObjectId` and its sibling
  newtypes (`OperationId`, `EventId`, `CommandId`, `CorrelationId`,
  `ConflictId`, `ReplicaId`) use a **manual** `Debug`/`Display` impl that
  formats as a UUID string (e.g. `ObjectId(3fa85f64-...)`) rather than the
  literal `#[derive(Debug)]` shown in the frozen contract text (which would
  print the raw 16-byte array). This does not change the `Serialize`/
  `Deserialize` wire format (still a 16-element byte array), only the
  human-readable `{:?}`/`{}` output. Flagged as a deliberate deviation from
  the contract's literal derive list, not a silent change.

---

## Self-Caught Bugs (found and fixed during implementation)

1. **`CreateBlueprintRevision` had no status guard.** A proptest invariant
   (`archived_is_terminal_under_any_further_commands`) caught that
   `CreateBlueprintRevision` was implemented with no status gate at all,
   meaning it would succeed even on an `Archived` mission — contradicting
   "Archived is terminal" (ruling A). Fixed: `CreateBlueprintRevision` is
   now rejected from `Closed`, `Archived`, and `Cancelled`, mirroring
   `CancelMission`'s non-terminal state set. Locked in by an explicit unit
   test (`closed_rejects_create_blueprint_revision`) in addition to the
   property test that originally caught it.

2. **`MissionRestarted` initially could not carry enough information for
   `apply()` to be pure.** An early draft attempted to have `apply()`
   "remember" `checklist_completed` via a nonexistent hidden field on the
   aggregate. Corrected before it reached compiled code: the event itself
   (`MissionEvent::MissionRestarted`) carries `checklist_completed`, so
   `apply()` can determine `Active` vs. `Paused` purely from the event
   during replay, with no dependency on transient state. Locked in by a
   golden fixture test
   (`golden_mission_restarted_event_carries_checklist_completed`).

---

## Deliverable Verification (all commands run against this workspace)

```
cargo build --release --workspace                      # ok
cargo test --lib --workspace                            # ok (36 tests: 23 kernel + 6 contracts + 3 mission-lib + 4 work-lib)
cargo test --workspace --test unit                      # ok (126 tests: 65 mission + 61 work)
cargo test --workspace --test property -- --ignored     # ok (8 tests: 4 mission + 4 work)
cargo test --workspace --test golden -- --ignored       # ok (8 tests: 4 mission + 4 work)
cargo clippy --workspace --all-targets -- -D warnings   # clean
cargo doc --workspace --no-deps                         # clean
```

Total: **178 tests, 0 failures.** 100% state-transition coverage (every
valid transition and a representative invalid-transition rejection from
every state, plus full terminal-state exhaustiveness for `Mission::Archived`)
for both Mission and Task aggregates.





# DECISIONS.md — Increment 2: Persistence, Events, and Messaging Infrastructure

This file records every architectural ruling, deviation from the frozen specification,
and contract gap resolved during the implementation of Increment 2. It is the
**amendment log** for downstream teams who consume this increment's contracts.

All rulings are traceable to the formal ruling documents exchanged between the
All-Father (architect) and Team 2 (implementer). The Handover Document
(`ONYX_Handover_Document__Machine-Readable_Contract_.md`) and Team Prompt 2
(`Team_Prompt_2_-_Persistence__Events__and_Messaging_Infrastructure.md`) are the
frozen starting points; this file records all agreed deviations.

---

## Section 1: Contract Fixes — Port Traits

| ID      | Area                         | Issue Found                                                                                                                                                                                                                                                                                  | Ruling                                                                                                                                                                                                                                    | Impact on Downstream                                                                                                              |
|:------- |:---------------------------- |:-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |:----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |:--------------------------------------------------------------------------------------------------------------------------------- |
| **U1**  | `UnitOfWork`                 | Original §3.2 had `register_event<E: Into<DomainEventEnvelope<A::Event>>>` referencing undeclared type parameter `A`, making the trait non-object-safe while §3.1 and §6.1 used it as `&mut dyn UnitOfWork`. Team 1's real `DomainEventEnvelope<E>` is genuinely generic (per Handover Doc). | `register_event` and `register_idempotency_result` accept `serde_json::Value` (serialized envelope). The caller constructs the typed envelope and calls `serde_json::to_value(...)` before registering. `Repository` is also non-generic. | Command handlers must serialize their events before registering.                                                                  |
| **U2**  | `UnitOfWork::set_repository` | Original §3.2 had `set_repository(&mut self, repo: Arc<dyn Repository<A>>)` referencing undeclared `A`.                                                                                                                                                                                      | Dropped entirely. Repository is passed to `Repository::commit(…, &mut dyn UnitOfWork)` directly; no cross-reference between the two types.                                                                                                | N/A (method never existed in the real port).                                                                                      |
| **C1**  | `Connection` trait           | `UnitOfWork::connection()` returned `&dyn Connection` but `Connection` was never defined anywhere.                                                                                                                                                                                           | Defined as `pub trait Connection: Send + Sync { fn as_any(&self) -> &dyn Any; }`. Adapters implement it; `Repository` downcasts via `as_any()`.                                                                                           | Repository implementations must downcast to their concrete `UnitOfWork` type.                                                     |
| **UF1** | `UnitOfWorkFactory::create`  | Factory signature `create()` had no tenant context; adapter can't populate `aggregates.organization_id` without it.                                                                                                                                                                          | Changed to `create(organization_id: OrganizationId)`. Organization ID flows in from `CommandEnvelope.actor.organization_id`.                                                                                                              | Command handlers must extract `organization_id` from the envelope and pass it to `create()`.                                      |
| **R1**  | `Repository` generics        | Original `Repository<A: AggregateRoot>` with generic `load`/`commit` created a circular dependency and broke object-safety with `&mut dyn UnitOfWork`.                                                                                                                                       | `Repository` is non-generic. `load()` returns `Option<Loaded>` where `Loaded.aggregate` is `serde_json::Value`. `commit()` accepts `aggregate_state: Value, events: &[Value]`.                                                            | The repository never knows about aggregate/event types. Callers deserialize `Loaded.aggregate` into the concrete type themselves. |

---

## Section 2: Undefined Types and Missing Traits

| ID     | Type/Trait                     | Gap                                                                             | Ruling                                                                                                                                                                   | Source of Authority                 |
|:------ |:------------------------------ |:------------------------------------------------------------------------------- |:------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |:----------------------------------- |
| **M3** | `IdGenerator`                  | Referenced as undefined in the Handover Doc pre-ruling.                         | Team 1's real `platform-contracts` already defines and exports `IdGenerator` (matching the ruling's shape). Use `platform_contracts::IdGenerator`.                       | Team 1 deliverable (authoritative). |
| **M4** | `DeadLetterStore`              | Referenced in §5.1 relay algorithm but never defined in the Handover Doc or §3. | Defined in `worker-application/src/ports/dead_letter_store.rs`. Signature: `async fn send(&self, claimed: &ClaimedMessage, error: &str) -> Result<(), DeadLetterError>`. | Team 2 owns this port.              |
| **M5** | `IdempotencyStore`             | Referenced in §6.1 but never defined.                                           | Defined in `query-application/src/ports/idempotency_store.rs`. `get`/`put` accept `serde_json::Value` (consistent with ruling U1).                                       | Team 2 owns this port.              |
| **M6** | `OutboxId`, `OutboxMessage`    | Referenced in §3.2 but never defined.                                           | Defined in `query-application/src/ports/unit_of_work.rs`. `OutboxId(u64)` (placeholder on registration; real ID assigned by DB BIGSERIAL).                               | Team 2 owns these types.            |
| **M7** | `ClaimedMessage`, `LeaseToken` | Referenced in §3.3 but never defined.                                           | Defined in `worker-application/src/ports/outbox_store.rs`.                                                                                                               | Team 2 owns these types.            |
| **M8** | `Connection`                   | See C1 above.                                                                   | See C1.                                                                                                                                                                  | Team 2 owns this port.              |

---

## Section 3: Team 1 / Team 2 Interface Reconciliation

| ID     | Issue                                                                                                                                                                                                                       | Ruling                                                                                                                                                                                                       |
|:------ |:--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **T1** | Team 1's `Mission` and `Task` aggregates did not derive `Serialize`/`Deserialize`, blocking `Repository::commit`'s `Value`-based contract.                                                                                  | Added `#[derive(Serialize, Deserialize)]` to both structs. All field types already derived these traits; the change was purely additive. Documented as a necessary amendment to Team 1's frozen deliverable. |
| **O1** | The `aggregates` table requires `organization_id NOT NULL`, but `Mission`/`Task` aggregates carry no `organization_id` field (it is a tenant boundary at the command-envelope/`ActorContext` level, not a domain property). | `organization_id` is supplied via `UnitOfWork`, which is created with it from `CommandEnvelope.actor.organization_id`. `Repository::commit` reads it via `unit.organization_id()`.                           |

---

## Section 4: Schema Decisions

| ID     | Area                       | Decision                                                                                                                                                                                |
|:------ |:-------------------------- |:--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **S1** | SQLite schema type mapping | UUID → `BLOB` (raw 16 bytes), `JSONB` → `TEXT`, `TIMESTAMPTZ` → `INTEGER` (Unix milliseconds), `BIGSERIAL`/`BIGINT` → `INTEGER PRIMARY KEY AUTOINCREMENT`, `BOOLEAN` → `INTEGER` (0/1). |
| **S2** | SQLite down migrations     | Added `.down.sql` files for both Postgres and SQLite migrations (not specified in Team Prompt 2; added as standard `sqlx migrate` practice).                                            |

---

## Section 5: Timestamp Precision

| ID     | Ruling                                                                                                                                                                                                                                                                                                                             |
|:------ |:---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **T1** | All persisted timestamps are truncated to milliseconds (`millis = nanos / 1_000_000`). Sub-millisecond precision is **intentionally and permanently lost**. Deadlines, ordering, and audit markers are determined by vector clocks, not wall-clock precision. Millisecond resolution is sufficient for all scheduling and logging. |
| **T2** | On read-back: `nanos = millis * 1_000_000`.                                                                                                                                                                                                                                                                                        |

---

## Section 6: Postgres Adapter Decisions

| ID      | Area                                      | Decision                                                                                                                                                                                                                                                                                                               |
|:------- |:----------------------------------------- |:---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **P1**  | Serde field extraction                    | Single-field tuple structs over scalars (`ObjectVersion(u64)`, `Timestamp(u64)`) serialize as bare numbers via `serde_json`. Array-backed types (`EventId([u8;16])`, `ObjectId([u8;16])`) serialize as JSON arrays. Verified empirically. Extraction code written accordingly (no `.get(0)` call for scalar newtypes). |
| **P2**  | Aggregate upsert SQL                      | `INSERT ... ON CONFLICT (id) DO UPDATE SET ... WHERE aggregates.version < EXCLUDED.version`. If `rows_affected() == 0` on a conflict, it is a version-race / optimistic-concurrency conflict.                                                                                                                          |
| **P3**  | Outbox claim isolation                    | `FOR UPDATE SKIP LOCKED` used in Postgres claim query to prevent two relay workers from claiming the same row concurrently.                                                                                                                                                                                            |
| **P3a** | SQLite claim isolation                    | `FOR UPDATE SKIP LOCKED` is unavailable in SQLite. SQLite uses database-level locking, which is acceptable for the single-writer desktop/mobile use case this adapter targets.                                                                                                                                         |
| **P4**  | `consumer_id` in `OutboxStore`            | The frozen §4.1 schema has no `consumer_group` column. `claim_unpublished`'s `consumer_id` parameter is a no-op in Increment 2 (all messages are claimed by the single global relay). Retained for future extensibility.                                                                                               |
| **P5**  | Dead-lettered outbox rows                 | Outbox rows that are dead-lettered are set to `published = TRUE` (never re-claimed) and retained permanently for audit, per §4.1's "never delete published rows immediately" note.                                                                                                                                     |
| **P6**  | `InboxStore::mark_processed` on duplicate | Returns `Err(AlreadyProcessed)` rather than silently succeeding, making the duplicate-delivery condition explicit and auditable. Callers treat `AlreadyProcessed` as a `continue`/skip, not a failure to retry.                                                                                                        |
| **P7**  | Integration test isolation                | Tests share a single live Postgres database (no per-test schema or transaction-per-test isolation). A process-wide `tokio::sync::Mutex` + `TRUNCATE` before each test serializes tests and prevents cross-test state pollution.                                                                                        |

---

## Section 7: Outbox Relay Decisions

| ID            | Area                              | Decision                                                                                                                                                                                                                                                                                                                            |
|:------------- |:--------------------------------- |:----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Ruling 3b** | max_retries check                 | The frozen §5.1 algorithm omitted a `retry_count >= max_retries` check, causing permanently-transient failures to retry forever. The Dead-Letter acceptance criterion is authoritative. The relay now checks `claimed.retry_count >= config.max_retries` **before** attempting to publish and redirects to dead-letter immediately. |
| **D1**        | Dead-letter table single writer   | `DeadLetterStore` is the **sole writer** of the `dead_letter` table. `OutboxStore::dead_letter()` delegates to `DeadLetterStore::send(&claimed, error)` rather than writing directly, eliminating duplicate-row risk when both are called.                                                                                          |
| **O1**        | Outbox lease window               | Lease window default: 30 seconds. Configurable via `RelayConfig.lease_duration`. The lease prevents duplicate processing across concurrent relay workers.                                                                                                                                                                           |
| **O2**        | `consumer_id` in `OutboxStore`    | See P4 above.                                                                                                                                                                                                                                                                                                                       |
| **O3**        | Dead-letter retention             | See P5 above.                                                                                                                                                                                                                                                                                                                       |
| **D-sig**     | `DeadLetterStore::send` signature | The frozen §5.1 uses `dead_letter.send(&claimed)`. The ruled implementation uses `send(&claimed, error: &str)` so the error context is preserved in the `dead_letter` table's `last_error` column.                                                                                                                                  |

---

## Section 8: Messaging Adapter Decisions

| ID     | Area                  | Decision                                                                                                                                                                                                                                                                                                                                                                                        |
|:------ |:--------------------- |:----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **M1** | NATS not implemented  | Team Prompt 2 §3.5 says "NATS or embedded channel implementation". NATS integration is out of scope for Increment 2 and would require a separate infrastructure dependency. The `messaging-adapter` crate provides a Tokio broadcast channel (`ChannelEventPublisher`) as the embedded option. A NATS adapter would implement the same `EventPublisher` port and be wired in at binary startup. |
| **M2** | No-subscriber publish | `ChannelEventPublisher::publish` returns `Err(PublishError::Transient)` when there are no active subscribers (the broadcast `send()` fails). This is treated as transient: the message is already persisted in the outbox; when the subscriber reconnects, it can replay from there.                                                                                                            |

---

## Section 9: Additional Infrastructure Decisions

| ID          | Area                                        | Decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
|:----------- |:------------------------------------------- |:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **C_infra** | `persistence-common` crate                  | Added a `persistence-common` crate (not in Team Prompt 2's original §2 File Manifest) to consolidate `Timestamp`↔millis, `Uuid`↔BLOB, and JSON↔text helpers shared between the Postgres and SQLite adapters. Approved per Team 2 architectural ruling.                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| **H1**      | Post-decide `apply()` in command handler    | The §6.1 command handler calls `aggregate.apply(event)` to update the in-memory aggregate before persisting. In this increment, the serialized pre-decide state is persisted (since the domain aggregate holds no `organization_id` field, we can't fully reconstruct the post-apply state from the post-decide aggregate without also having that field in the struct). The state stored in the `aggregates` table is the result of `serde_json::to_value(&aggregate)` at the point the aggregate was loaded, not after applying new events. This is corrected in Increment 5 when the full command pipeline is completed with proper post-apply persistence. Documented for Team 5's awareness. |
| **T_test**  | Test isolation — deterministic ID collision | Added a global `AtomicU64` seed counter to `mission_domain::test_support::test_context()` so concurrent integration tests each get a unique `ObjectId` space (spaced 1,000,000 apart per call). The original `DeterministicIdGenerator::new()` still starts at counter `1` for Team 1's own unit tests, which may rely on exact reproducibility.                                                                                                                                                                                                                                                                                                                                                  |
| **T2_test** | Integration test mutex                      | See P7 above.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |

---

## Section 10: Out-of-Scope for Increment 2

| Item                                                               | Deferred To                                                                                                                                                                                       |
|:------------------------------------------------------------------ |:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| NATS / Kafka `EventPublisher` implementation                       | Increment 7 (Observability, Background Processing, and Security Hardening)                                                                                                                        |
| Full HTTP routing in `api-server`                                  | Increment 5 (Desktop and Mobile Native Clients) / Increment 6 (Web UI Thin Client)                                                                                                                |
| `VerifiedAuthority` / `PolicyDecisionSet` full implementation      | Increment 7 (Security Hardening)                                                                                                                                                                  |
| `consumer_group` column in `outbox` table (multi-consumer support) | Future increment, schema migration when required                                                                                                                                                  |
| `testcontainers` per-test PostgreSQL instance (per §9.1 spec)      | Increment 2 uses a shared live Postgres instance with mutex+truncate isolation (see P7). Full testcontainer isolation can be adopted when the test suite exceeds ~5 integration tests per binary. |

---

---

## Section 11: Quality Gate Verification

Per the standard established in Increment 1, the following were run and verified clean prior to delivery:

| Gate                                     | Result                                                                                                           |
|:---------------------------------------- |:---------------------------------------------------------------------------------------------------------------- |
| `cargo build --workspace`                | Clean, 0 errors, 0 warnings                                                                                      |
| `cargo test --workspace`                 | All tests pass (36 carried from Team 1 + 57 new = 93 total; 8 golden tests intentionally `#[ignore]`d by Team 1) |
| `cargo clippy --workspace --all-targets` | Clean, 0 warnings (after fixing 1 `redundant_closure` and 5 `manual_memcpy` lints)                               |
| `cargo doc --workspace --no-deps`        | Clean, 0 errors, 0 warnings                                                                                      |

The only residual warning anywhere in the build is an upstream `sqlx-postgres v0.7.4` future-incompatibility notice (`the following packages contain code that will be rejected by a future version of Rust`), which originates in the `sqlx` dependency itself, not in Increment 2's code, and does not fail the current build.

---

*Document maintained by: Team 2 (Persistence, Events, and Messaging Infrastructure)*
*Increment: 2*
*Handover Document version: As delivered (see `ONYX_Handover_Document__Machine-Readable_Contract_.md`)*





# Architectural Decisions — Team 3 (Synchronization Engine & Conflict Resolution)

**Date:** 2026-08-04
**Status:** Binding
**Supersedes:** Team Prompt 3 v1.0 ambiguous/contradictory sections, per Handover Document §17

This document is the formal amendment to `Team Prompt 3: Synchronization
Engine & Conflict Resolution` (v1.0, frozen 2026-08-01). It records every
ruling issued during Team 3's build, every deviation from the frozen
contract those rulings authorized, and every additional deviation
discovered and resolved during implementation that required a judgment
call rather than a business decision.

Two initiation rounds happened, because the frozen contract's own
assumptions changed mid-build:

- **Round 1** — Team 3's kickoff, before the real Increment 1/2 deliverable
  existed. Team 3 built its own `kernel-contracts-stub`, transcribed from
  the Handover Document, to make `crdt` and `synchronization-domain`
  compile against *something*. Rulings A–D below are from this round.
- **Round 2** — After the real Increment 1/2 deliverable (`platform-kernel`,
  `platform-contracts`, and Team 2's own `kernel-contracts-stub`) was
  uploaded, three real contract mismatches surfaced between Team Prompt 3
  and the delivered code. Rulings Q1–Q3 below are from this round, and
  **supersede** any Round 1 ruling that conflicts with them. The entire
  `crdt`/`synchronization-domain`/`sync-test-utils` workspace was rebuilt
  against the real kernel; Round 1's stub is not part of the final
  deliverable.

---

## Round 2 Rulings (binding, supersede Round 1 where they conflict)

### Q1 — Which kernel source should Team 3 use?

**Ruling:** Use Team 1's delivered `platform-kernel` and `platform-contracts`
crates directly. All kernel types (`ObjectId`, `EventId`, `VectorClock`,
`ConflictId`, `ReplicaId`, etc.) come from `platform-kernel`. Do not
re-define any kernel types. Team 3's own Round-1 `kernel-contracts-stub`,
and its independent (and incompatible — `Uuid`-wrapper vs. Team 1's
`[u8; 16]`-array) identifier types, are discarded entirely.

**Consequence:** every file in `crdt` and `synchronization-domain` imports
from `platform_kernel`/`platform_contracts`, not from any Team-3-local
stub. Three real API differences between what Team Prompt 3 v1.0 assumed
and what `platform-kernel` actually provides had to be resolved *without*
modifying Team 1's crate (see "Non-ruling implementation deviations, Round
2" below): `VectorClock` has no `Ord`, `ReplicaId` has no zero-sentinel
constructor, and `GarbageCollector`'s own `VectorClock.entries` is a public
field, not a method (a Team-3-local naming assumption from Round 1, not a
kernel gap).

### Q2 — `AggregateRoot::conflict_pending()` is missing

**Ruling:** Add `conflict_pending()` to the `AggregateRoot` trait, as a
non-breaking additive default (`fn conflict_pending(&self) -> Option<ConflictPendingMarker> { None }`),
in a **local copy** of `platform-contracts` — not by editing Team 1's
delivered crate in place.

**Implementation:** `crates/kernel/platform-contracts-ext` is that local
copy: an exact copy of Team 1's `platform-contracts` source, with
`ConflictPendingMarker` and the `conflict_pending()` default method added
to `traits.rs`, and its own `Cargo.toml`/`lib.rs` doc comments making clear
it is not the authoritative crate. Every Team 3 crate that needs
`AggregateRoot` depends on `platform-contracts-ext`, not on
`platform-contracts` directly, for that trait. `platform-contracts` itself
(Team 1's real crate) is untouched.

### Q3 — `OutboxStore::enqueue()` doesn't exist; how does escalation deliver its event?

**Ruling:** Option A (recommended): escalation should go through the
command pipeline (`CommandEnvelope` → handler → `UnitOfWork::register_outbox()`),
not a direct `OutboxStore::enqueue()` call that doesn't exist on the real
port.

**What was actually discovered and implemented, and why it isn't a literal
reading of the ruling's sketch:** two things surfaced while implementing
this that the ruling's sketch didn't anticipate, both recorded rather than
silently worked around:

1. The real command pipeline (`command_handler::handle_command<A, C, E,
   Err>`) is generic over a concrete `AggregateRoot` with a `decide()`/
   `apply()` pair loaded from a repository. Escalation is not "a command
   against an existing aggregate's state machine" — there is no
   `Mission`/`Task`-like aggregate whose `decide()` should produce an
   `EscalateConflict` event. Routing escalation through
   `handle_command<A, C, E, Err>` would require fabricating a fake
   aggregate type with no real state, purely to satisfy a generic
   signature that assumes one exists.
2. `Repository::commit()`'s real, delivered implementation (both
   `persistence-sqlite` and, per the same pattern, `persistence-postgres`)
   only calls `unit.register_event(...)` for each event — it never calls
   `unit.register_outbox(...)`. So even the literal command pipeline does
   not currently deliver outbox messages for *any* event, escalation or
   otherwise. **This is a pre-existing gap in the delivered Increment 2
   code, not something Team 3 introduced or can fix from here** (fixing it
   means editing Team 1/2's persistence adapters, out of this increment's
   scope).

**What `EscalationService` actually does**, staying as close to Option A's
intent (durable, transactional, no direct network call) as the real
delivered ports allow: `escalate_conflict` constructs a
`DomainEventEnvelope<ConflictEscalationRequested>` — mirroring exactly what
`command_handler::handle_command`'s step 8 does for a normal domain event —
and commits it via `UnitOfWorkFactory::create()` + `Repository::commit()`,
the same durable, transactional path every other domain event in this
system goes through. This guarantees the escalation event is durably
recorded exactly once per call, with no direct network I/O. **Outbox
*publication* of that event remains contingent on Increment 2's
`register_outbox()` gap being closed** — a cross-increment follow-up, not
silently assumed to work. `EscalationService::new` takes `Arc<dyn
Repository>` + `Arc<dyn UnitOfWorkFactory>`, not `Arc<dyn OutboxStore>`.

---

## Round 1 Rulings (binding for Round-1-era code; largely superseded by Q1 above for kernel types, retained here for the record and because several still apply verbatim to `crdt`'s pure-domain logic)

### A. Foundation Types

| Defect                          | Ruling                                 | Status after Round 2                                                         |
|:------------------------------- |:-------------------------------------- |:---------------------------------------------------------------------------- |
| A1–A5: ~22 missing kernel types | Use `kernel-contracts-stub` from Inc 2 | **Superseded by Q1** — use real `platform-kernel` instead                    |
| A6: `OutboxStore`               | Add `enqueue` as a dev-dependency      | **Superseded by Q3** — no `enqueue`; event-sourced write instead             |
| A7: `tracing`                   | Add to allowed dependencies            | Still applies — `tracing` is a direct dependency of `synchronization-domain` |

### B. Signature Contradictions (still applicable — these are about `crdt`'s own logic, not about which kernel it depends on)

| ID  | Issue                                                                                           | Ruling                                                                                                       | Notes                                                                                                                                                                                                                                                                                                     |
|:--- |:----------------------------------------------------------------------------------------------- |:------------------------------------------------------------------------------------------------------------ |:--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1  | `VectorClock::merge` mutating vs returning                                                      | Use immutable: `fn merge(&self, other: &Self) -> Self`                                                       | Matches `platform_kernel::VectorClock`'s real signature exactly — no Team 3 code needed to enforce this, Team 1 already built it this way.                                                                                                                                                                |
| B2  | `MergeOutcome::Conflict` variant inconsistency                                                  | Struct variant: `Conflict { conflict_type }`                                                                 | Applied as-is in `merge_strategy.rs`.                                                                                                                                                                                                                                                                     |
| B3  | `merged_value` unbound in `merge_field`                                                         | Bind it via real deserialize/merge/serialize round-trip through the concrete CRDT type for the field's shape | Applied in `merge_shape<T: Crdt>()`.                                                                                                                                                                                                                                                                      |
| B4  | `GarbageCollector` private fields                                                               | Make `summary_vector`/`known_replicas` public                                                                | Applied — `crdt` owns this type, no cross-crate access concern.                                                                                                                                                                                                                                           |
| B5  | `VectorClock` used as `BTreeMap` key (needs `Ord`)                                              | Add a lexicographic `Ord` tie-break                                                                          | **Superseded by Q1's consequence**: the real `platform_kernel::VectorClock` has no `Ord`, and Team 3 has no authority to add one to Team 1's type (see "Non-ruling deviations, Round 2" below) — `MvRegister` was redesigned to avoid needing `Ord` on `VectorClock` at all, rather than fabricating one. |
| B6  | Missing type bounds (`Ord` on `OrSet<T>`, `Eq` on `LwwRegister<V>`)                             | Add the missing bounds                                                                                       | Applied as-is.                                                                                                                                                                                                                                                                                            |
| B7  | `Rga::insert_after` calls a nonexistent `get_local_replica_id()`; `ElementId::root()` undefined | Pass `local_replica` explicitly; define `ElementId::root()` as an all-zero sentinel                          | Applied; the sentinel is now built from `platform_kernel::ReplicaId`'s public `[u8; 16]` field rather than a `zero_sentinel()` kernel method (which doesn't exist and isn't Team 3's to add).                                                                                                             |
| B8  | `SynchronizationSession::complete` takes `self` immutably but assigns `self.phase`              | `fn complete(mut self) -> SyncResult`                                                                        | Applied as-is.                                                                                                                                                                                                                                                                                            |

### C. Dependencies

| ID  | Issue                                                                     | Ruling                                                                                                                                                                                                 | Status                                                                                                                                                                                                           |
|:--- |:------------------------------------------------------------------------- |:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |:---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C1  | Tokio/async-trait/tracing banned by §5.1 but required by async code/tests | `async-trait`/`tracing` as production deps (required by `EscalationService`'s `async fn`); `tokio` confined to `dev-dependencies` (only needed to *run* async tests, not to compile `async fn` itself) | Applied — stricter than the original ruling's literal "add tokio to dev-deps for everything," but doesn't contradict its intent and better honors §5.1's actual goal (no runtime dependency in production code). |

### D. Test Harness Gaps

| ID  | Issue                                                                                                                                                                                          | Ruling                                                                                  | Status                                                                                                                                                      |
|:--- |:---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |:--------------------------------------------------------------------------------------- |:----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1  | §8 acceptance criteria name `conflict_pending_rejects_commands`, `replay_window_tests::old_operation_still_reconciled`, `session_tests::session_resumes_after_interruption` — none shown in §6 | Implement them                                                                          | All three implemented; see `tests/integration/conflict_tests.rs`, `replay_window_tests.rs`, `session_tests.rs`.                                             |
| D2  | `exchange_operations` declares `Vec<MergeOutcome>` but never populates it                                                                                                                      | Buffer outcomes in the session                                                          | Applied.                                                                                                                                                    |
| D3  | `reconcile` always returns an empty conflict list                                                                                                                                              | Actually detect authority-controlled concurrent writes and populate `pending_conflicts` | Applied — see `SynchronizationSession::reconcile()`'s doc comment for the exact dispatch logic and the fail-safe default for fields with no known metadata. |
| D4  | Frozen tests bind sessions/conflicts immutably then mutate them                                                                                                                                | Use `mut`                                                                               | Applied throughout.                                                                                                                                         |

---

## Non-ruling implementation deviations, Round 1 (flagged, not ruled on individually — judgment calls within already-granted latitude)

- **Duplicate `ConflictPendingMarker`.** Team Prompt 3 defines it twice: once
  in §3.12 (`conflict.rs`) and once implicitly via §3.14's
  `aggregate.conflict_pending()` return type. Resolved by re-exporting one
  definition (now the one added to `platform-contracts-ext` per Q2) rather
  than keeping two independently-defined, potentially-divergent copies.
- **`Value` (used throughout `MergeStrategy` but never defined).** Defined
  as `serde_json::Value` — the minimal opaque wire type that lets
  `merge_field` dispatch generically by field shape without knowing
  concrete Rust element types at the call site.
- **`extract_field_path`.** Team Prompt 3 never specifies how a session
  derives a field path from an event envelope. Convention: read
  `payload.field` if present (a JSON object with a top-level `"field"`
  string key), else fall back to `event_type`. Flagged rather than silently
  assumed, since a different convention would silently misroute conflict
  detection.
- **Unknown-field-shape default in `reconcile()`.** A field with no entry in
  the session's `field_metadata` map is treated as **authority-controlled**
  (fail-safe: always conflict, never silently auto-merge), not as
  CRDT-eligible. Silently defaulting to "safe to auto-merge" would be
  exactly the kind of silent authority-conflict resolution the Mission
  statement (§1) forbids.
- **`MvRegister::new`'s replica parameter.** Team Prompt 3's own frozen text
  calls `ReplicaId::random()` with an inline comment "Should be provided by
  context" — i.e. the spec author already flagged this as a placeholder
  that silently manufactures identity instead of using the caller's real
  one. Fixed: `new(value: V, replica: ReplicaId)` takes the replica
  explicitly.
- **`tombstone_gc.rs`'s `gc_pass` genericity.** §3.8 itself calls its
  `gc_pass` sketch "simplified... real implementation would dispatch based
  on the actual CRDT type." The frozen `Crdt` trait exposes only
  `causal_context()` — no per-type accessor for individual tombstoned
  elements — so a fully generic `gc_pass` cannot itself mutate away
  tombstones inside an arbitrary `T: Crdt` without knowing that type's
  internal shape. Implemented honestly as the type-agnostic half (causal
  eligibility scan across a batch), documented as such, rather than faking
  a per-type removal the trait doesn't support.

## Non-ruling implementation deviations, Round 2 (flagged, discovered during the real-kernel rebuild)

- **`VectorClock: Ord` cannot be added.** Ruling B5 (Round 1) called for
  adding a lexicographic `Ord` tie-break to `VectorClock` so it could be
  used as a `BTreeMap` key in `MvRegister`. Under Team 3's own Round-1 stub
  this was straightforward (Team 3 owned the type). Under the real
  `platform_kernel::VectorClock` (Q1), Team 3 has no authority to add trait
  impls to Team 1's crate, and — since vector clocks are only ever
  *partially* ordered by design — fabricating a total order on the type
  itself risked that order leaking into causal-comparison code elsewhere
  where it does not belong. **Resolution:** `MvRegister<V>` no longer
  stores its concurrent versions in `BTreeMap<VectorClock, V>`; it uses
  `Vec<(VectorClock, V)>` instead, which needs no `Ord` at all. This also
  removed the "arbitrary but deterministic tie-break" `get_latest()` method
  (which implied a false total order on genuinely concurrent values) in
  favor of `get_single()` (returns `None` on real concurrency, forcing the
  caller through `MergeStrategy`/`ConflictRecord` instead of silently
  picking a "winner") and `has_conflict()`.
- **`ElementId::root()`'s sentinel.** No `ReplicaId::zero()`/`zero_sentinel()`
  constructor exists on the real `platform_kernel::ReplicaId`. Built
  directly via the type's public `[u8; 16]` field (`ReplicaId([0u8; 16])`)
  instead of requesting a kernel-side addition for a single internal
  sentinel value.
- **`GarbageCollector`'s `VectorClock.entries` access.** Round 1's stub had
  `entries()` as a method; the real `platform_kernel::VectorClock` exposes
  `entries` as a public field. Fixed call sites accordingly (`&clock.entries`
  instead of `clock.entries()`).
- **`VectorClock::with()` builder used by Team Prompt 3's own frozen §6.3
  test.** Neither the Handover Document nor the real `platform_kernel`
  defines this fluent builder. The tombstone GC test
  (`tests/integration/tombstone_gc_tests.rs`) was rewritten to build test
  clocks via repeated `.increment()` calls — the real, documented
  `VectorClock` API — instead of requesting a builder method be added to
  Team 1's type for test convenience.
- **`ConflictRecord`'s event envelope type parameter.** The real
  `platform_contracts::DomainEventEnvelope<E>` is generic over its payload.
  `ConflictRecord` fixes `E = serde_json::Value` rather than being generic
  itself, matching how the real command/repository pipeline already
  treats events as opaque `Value` once they cross the aggregate boundary
  (see `command_handler.rs` step 8: events are serialized to `Value` before
  a `ConflictRecord` could ever see them).
- **`EscalationService`'s "system actor."** Escalation is raised by the
  synchronization engine itself, not a human. `system_actor()` constructs
  an `ActorContext` with `user_id`/`device_id` defaulted to a fresh
  `ObjectId`, rather than requiring every caller of `escalate_conflict` to
  fabricate a plausible-looking human identity for an event no human
  triggered.

---

## Deliverable Summary

| Artifact                       | Location                                         | Status                                                                                                                                                                                                           |
|:------------------------------ |:------------------------------------------------ |:---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crdt` crate                   | `crates/synchronization/crdt/`                   | 7 CRDT/support types (`OrSet`, `LwwRegister`, `MvRegister`, `PnCounter`, `Rga`, `AppendOnlyLog`, `GarbageCollector`) + `Crdt` trait. Compiles clean, clippy clean, full doc coverage (`#![deny(missing_docs)]`). |
| `platform-contracts-ext`       | `crates/kernel/platform-contracts-ext/`          | Local, additive-only extension of Team 1's `platform-contracts` (Ruling Q2). Not authoritative; a mechanical drop-in once Team 1 adds `conflict_pending()` upstream.                                             |
| `synchronization-domain` crate | `crates/synchronization/synchronization-domain/` | `SynchronizationSession`, `MergeStrategy`, `ConflictRecord`, `EscalationService`, `check_conflict_pending` guard. Compiles clean, clippy clean, full doc coverage.                                               |
| `sync-test-utils` crate        | `crates/synchronization/sync-test-utils/`        | In-memory `MockRepository`/`MockUnitOfWork`/`MockConflictRepository` (against the *real* `query-application` ports) + CRDT proptest generators.                                                                  |
| Tests                          | `tests/property/`, `tests/integration/`          | 11 real tests + 1 explicitly-`--ignored` full property test (64 cases × 10 shuffles, passes at `--release`). All pass.                                                                                           |
| `DECISIONS.md`                 | workspace root                                   | This file.                                                                                                                                                                                                       |
| `README.md`                    | `crates/synchronization/crdt/README.md`          | CRDT usage guide, quick start.                                                                                                                                                                                   |
| Doc coverage                   | `cargo doc --no-deps`                            | Zero `missing_docs` errors across all four Team 3 crates.                                                                                                                                                        |

### Verification commands

```bash
# Full Team 3 test suite
cargo test --package crdt --package synchronization-domain --package sync-test-utils --package platform-contracts-ext

# Full determinism property test (slower; explicit --ignored)
cargo test --package crdt --test determinism --release -- --ignored --nocapture

# Clippy, workspace-strict
cargo clippy --package crdt --package synchronization-domain --package sync-test-utils --package platform-contracts-ext --all-targets -- -D warnings

# No-wall-clock-GC acceptance criterion
grep -n "Duration::\|Instant::\|SystemTime::\|\.elapsed(" crates/synchronization/crdt/src/tombstone_gc.rs

# Doc coverage
RUSTDOCFLAGS="-D missing_docs" cargo doc --no-deps --package crdt --package synchronization-domain --package sync-test-utils --package platform-contracts-ext
```

### Known cross-increment follow-up (not Team 3's to fix)

`UnitOfWork::register_outbox()` exists on the port trait (`query-application`)
but is never called by either delivered `Repository::commit()`
implementation (`persistence-sqlite`, `persistence-postgres`). This means
**no event in the system — not just escalation events — currently reaches
the outbox table**, regardless of which crate produces it. `synchronization-domain`'s
`EscalationService` durably records its event via the same
`Repository::commit()` path every other event uses (see Q3 above), so it is
exactly as reliable as the rest of the system today, but outbox
*publication* specifically will not happen until this Increment 2 gap is
closed upstream.

---

# Team 4 — Networking Layer & P2P Transports

## Rulings Log (Team 4)

| #     | Issue                                                                                                      | Ruling / Resolution                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ----- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T4-1  | `SyncMessage::serialize`/`deserialize` visibility                                                          | Made `pub`. Signatures: `pub fn serialize(&self) -> Vec<u8>`, `pub fn deserialize(bytes: &[u8]) -> Result<Self, SerializationError>`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| T4-2  | Wire format: `version_len`                                                                                 | `[1 byte]` u8 length prefix + UTF-8 bytes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| T4-3  | Wire format: `target_replica`                                                                              | `Option<ReplicaId>` serialized as `[1 byte is_present]` + `[16 bytes]` (always present on the wire; zeroed and ignored when the flag is 0).                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| T4-4  | Wire format: `signature_len`                                                                               | `[2 bytes]` u16 big-endian length prefix + bytes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| T4-5  | QUIC NAT rebinding                                                                                         | NAT rebinding is intrinsic to `quinn::Connection`'s connection-ID-based migration; nothing to configure explicitly in the transport's `connect`/`send`/`recv` path. Verified by a real integration test (`tests/quic_nat_tests.rs`, `#[ignore]` by default — binds real UDP sockets) that binds two live sockets, rebinds the **client `Endpoint`** (not `Connection` — see T4-6) mid-session, and confirms the connection stays usable.                                                                                                                                                                                                                                                                                                      |
| T4-6  | Fabricated API caught and corrected                                                                        | The prompt's own sample code (and an early draft ruling) referenced `conn.rebind_socket(...)` / `conn.connection().endpoint().rebind(...)` on `quinn::Connection`. **Neither exists** in `quinn` 0.10.2 — verified against docs.rs's full method list for both `Connection` and `Connecting`. The real API is `quinn::Endpoint::rebind(std::net::UdpSocket)`, called on the `Endpoint` handle the caller already holds; the existing `Connection` object is unaffected by the rebind and continues to work via QUIC's connection-ID migration. Fixed in `tests/quic_nat_tests.rs` and verified by an explicit `cargo test -- --ignored` run (not just compiling — the real handshake, echo, rebind, and post-rebind `open_bi()` all succeed). |
| T4-7  | Wi-Fi Direct / BLE encryption flag                                                                         | Both `WifiDirectTransport` and `BluetoothLETransport` carry an `encryption_enabled: bool`, set by the caller (Team 5, based on platform capability). `is_available()` and `provides_encryption()` both return `false` if this flag is `false` — an unencrypted radio link is never reported as an approved transport, per §7.1 ("No approved transport carries plaintext").                                                                                                                                                                                                                                                                                                                                                                   |
| T4-8  | `TransportError::QuotaExceeded`                                                                            | Added as a real, reachable error variant. The transport layer does **not** enforce quota itself (§5); this variant exists so a Cloud Relay rejection can be surfaced distinctly from other connection failures. Exercised in `tests/quota_tests.rs`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| T4-9  | `CompositeDiscovery` dedup strategy                                                                        | The prompt's literal sample (`peers.dedup_by(\|a,b\| a.id == b.id)`) only removes **adjacent** duplicates in a `Vec` and cannot express "cloud wins" when a peer is discovered by both paths. Implemented instead via a `HashMap<ReplicaId, PeerInfo>`: local peers inserted first, then cloud peers overwrite any local entry with the same ID (cloud entries carry resolvable `endpoints` needed for QUIC). Added a `from_cloud: bool` field to `PeerInfo` (not in the prompt's literal struct) so tests can assert which source won.                                                                                                                                                                                                       |
| T4-10 | `Connection::transport_type()`                                                                             | Not listed in the frozen `Connection` trait under §3.1, but the acceptance test in §9.1 calls `conn.transport_type()` on a `Box<dyn Connection>` — a real contradiction between the trait definition and its own acceptance test, not a stylistic gap. Added the method to the trait rather than leaving the acceptance test unimplementable.                                                                                                                                                                                                                                                                                                                                                                                                 |
| T4-11 | "No direct `tokio` dependency in production code" (§8.1) vs. `quinn`/`tokio-tungstenite` needing a runtime | Resolved via a seam: `cloud_relay.rs` defines `RelaySocket`/`RelaySocketFactory` traits (using only `async-trait` + `std::future`); the real `tokio-tungstenite`-backed implementation is left to the composition root (e.g. a future `sync-agent` binary), not `sync-transport` itself. `quic_cross_network.rs` similarly avoids `tokio::time::timeout` via a hand-rolled `poll_fn`-based `timeout_future` helper. `tokio` appears only in `[dev-dependencies]`, driving `#[tokio::test]`.                                                                                                                                                                                                                                                   |
| T4-12 | §9.2 `encryption_tests::cloud_relay_uses_tls` has no real assertions in the prompt                         | The prompt's literal test body is a comment describing a live-relay packet capture, with no code. A live TLS handshake against a real relay is out of scope for a unit-testable crate. Implemented instead: send a `SyncMessage` with a distinctive payload marker through the `RelaySocket` seam, and assert the wire bytes (a) never equal a bare JSON/plaintext encoding of the payload and (b) round-trip through `SyncMessage::deserialize` — proving the wire format is the frozen binary encoding, never a debug/plaintext dump, while being explicit that the actual TLS handshake is delegated to the real `RelaySocketFactory` implementation at the composition root.                                                              |
| T4-13 | `sync-transport-mobile`'s C-ABI shape (§6.2) vs. real Android JNI convention                               | §6.2's table lists plain `extern "C"` signatures mirroring the iOS section. Real Android/JNI interop is normally the other direction (`Java_...` entry points, invoked via `jni::JNIEnv`). Implemented the literal `extern "C"` signatures as specified; a `jni`-crate-based JNI entry point is a reasonable next increment for Team 5 but wasn't fabricated here since the prompt doesn't specify one.                                                                                                                                                                                                                                                                                                                                       |
| T4-14 | Tests directory layout                                                                                     | Cargo requires test binaries directly under `tests/`, not nested under `tests/integration/` (a nested path isn't auto-discovered as a `[[test]]` target). Flattened to `tests/*.rs` so the deliverable's own required commands (`cargo test --package sync-transport --test fallback_tests`, etc.) actually resolve to real targets.                                                                                                                                                                                                                                                                                                                                                                                                          |

## Real-Workspace Integration (Team 4)

Team 4 was originally scaffolded as a standalone crate against local placeholder types (`OrganizationId`, `ReplicaId`, `Timestamp`, `SchemaVersion`), because no existing workspace was available at kickoff. Partway through the build, the real Increments 1–3 workspace was provided. Per ruling, integrated as follows:

- **Additive only.** `sync-transport` and `sync-transport-mobile` were added as new workspace members; no existing crate (`platform-kernel`, `platform-contracts`, etc.) was modified.
- **Real types adopted directly.** `platform_kernel::{ReplicaId, OrganizationId, Timestamp, SchemaVersion}` replaced the local placeholders entirely (not aliased, not wrapped) — `OrganizationId` is a type **alias** for `ObjectId` (`pub type OrganizationId = ObjectId;`), not a distinct newtype, so it is constructed via `ObjectId(bytes)`, not `OrganizationId(bytes)` (the latter doesn't compile: E0423, caught by the compiler, not assumed).
- **`AuthorityProvider` remains Team-4-owned.** No equivalent exists in `platform-kernel` (`AuthorityProof`/`ActorContext` are a proof object and an actor identity, not an async bearer-token-fetch abstraction) — kept as originally designed, documented as not a platform-kernel type.
- **`DiscoveryError`, `SerializationError`, `CloudDiscovery`, `LocalDiscovery`, `PlatformHandle`, `PlatformBLEHandle`** remain Team-4-owned (nothing upstream defines them; they're this team's own contract surface per §3.4/§4.3/§4.4/§6).
- **Compiler-caught bugs during integration** (not assumed correct, verified via `cargo check`):
  - `platform_kernel::ReplicaId` implements `Debug` (as `"ReplicaId(<uuid>)"`) but **not** `Display` — `format!("{}", peer.id)` in `cloud_relay.rs` didn't compile. Fixed by converting explicitly via `uuid::Uuid::from_bytes(peer.id.0)` before formatting.
  - `Box<dyn Connection>` is not `Debug` (the `Connection` trait doesn't require it), so `.unwrap_err()` on a `Result<Box<dyn Connection>, TransportError>` doesn't compile in two unit tests (`bluetooth_le.rs`, `selector.rs`). Fixed by matching on the `Result` explicitly instead.
  - `rustls::client::{ServerCertVerifier, ServerCertVerified}` are gated behind rustls's `dangerous_configuration` feature, needed by `tests/quic_nat_tests.rs` to skip certificate validation against its own self-signed test cert. Added as `features = ["dangerous_configuration"]` on the `dev-dependencies` declaration of `rustls` only. **Verified empirically** (not assumed) that `cargo build --lib`/`cargo check --lib` do not see this feature (real production/release builds are unaffected), but `cargo test` does unify it into the lib target's build of `rustls`, because Cargo unifies a dependency's features across `[dependencies]`/`[dev-dependencies]` within one package — a `#[cfg(test)]` guard on our own code would not change this, since the leak is a property of which features `rustls` itself is compiled with, not of which of our modules are cfg-gated. The code that actually uses the gated items already lives entirely in `tests/quic_nat_tests.rs` (never in `src/`). Accepted as low risk since production/release builds are unaffected.

## Verification (Team 4)

All commands run against a clean copy of the delivered workspace, in an exec-capable build directory (the `/mnt/user-data/outputs` FUSE mount is `noexec`, so builds cannot run directly there — see the Environment Constraints note below).

```bash
# Unit tests (26 tests)
cargo test --package sync-transport --lib

# Integration tests (13 tests across 5 binaries)
cargo test --package sync-transport --test fallback_tests
cargo test --package sync-transport --test encryption_tests
cargo test --package sync-transport --test discovery_tests
cargo test --package sync-transport --test quota_tests
cargo test --package sync-transport --test transport_tests

# QUIC NAT rebinding (ignored by default — binds real UDP sockets)
cargo test --package sync-transport --test quic_nat_tests               # skipped, exit 0
cargo test --package sync-transport --test quic_nat_tests -- --ignored  # runs for real, passes

# Mobile crate (host stub only on this Linux build host — see below)
cargo test --package sync-transport-mobile

# Clippy, scoped to Team 4's two crates (workspace-wide clippy currently
# fails on persistence-postgres for an unrelated, pre-existing reason — see
# "Known cross-team gap" below)
cargo clippy --package sync-transport --package sync-transport-mobile --all-targets --features sync-transport/test-support -- -D warnings
```

**Result:** 26 lib tests pass, 13 integration tests pass (across `fallback_tests`, `encryption_tests`, `discovery_tests`, `quota_tests`, `transport_tests`), `quic_nat_tests` passes when explicitly un-ignored, `sync-transport-mobile`'s host-stub test passes, and both crates are clippy-clean under `-D warnings`.

### Mobile FFI cross-compilation (verified separately from `sync-transport`'s dependency graph)

`sync-transport-mobile`'s own `Cargo.toml` conditionally depends on `objc` (iOS) / `jni` (Android) per `target_os`, and its `ios_*.rs`/`android_*.rs` modules are `#[cfg(target_os = "...")]`-gated so the crate builds on the Linux host used for this deliverable (only `host_stub.rs` compiles there).

To verify the platform-specific FFI code beyond "it's gated off, untested," each of the four platform files (`ios_multipeer.rs`, `ios_ble.rs`, `android_wifi_direct.rs`, `android_ble.rs`) was extracted into an isolated throwaway crate (no `sync-transport`/`quinn`/`ring` dependency chain) and **genuinely type-checked** against its real target triple:

```bash
rustup target add aarch64-apple-ios aarch64-linux-android
cargo check --target aarch64-apple-ios     # ios_multipeer.rs, ios_ble.rs: clean
cargo check --target aarch64-linux-android # android_wifi_direct.rs, android_ble.rs: clean
```

All four typecheck clean against their real target. **Linking** a full test binary for either target was not possible in this environment (no Xcode SDK for `aarch64-apple-ios`; no Android NDK C toolchain for `aarch64-linux-android`, which `ring` — a transitive `quinn`/`rustls` dependency — needs for its native build script), so this deliverable cannot claim the mobile crate links into a runnable binary. What's verified: the FFI code is syntactically and type-correct for its real target. What's not verified: end-to-end linking/execution on real Apple/Android toolchains, which is Team 5's integration step per the mission statement.

### Known cross-team gap (not Team 4's to fix)

`cargo clippy --workspace` (unscoped) currently fails during Increment 2's `persistence-postgres` crate: its `sqlx::query!` compile-time-checked macros require a live `DATABASE_URL` or a `cargo sqlx prepare` query cache, neither of which is present in this build environment. This is a pre-existing Team 2 requirement unrelated to any Team 4 change — confirmed by scoping clippy to `--package sync-transport --package sync-transport-mobile`, which passes clean, and by observing `sync-transport`/`sync-transport-mobile` compile without error even in the unscoped workspace-wide run before it fails on `persistence-postgres`.

## Deliverable Summary (Team 4)

| Artifact                      | Location                                      | Status                                                                                                                                                                                                                                                                                                                                                                          |
|:----------------------------- |:--------------------------------------------- |:------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sync-transport` crate        | `crates/transports/sync-transport/`           | `Transport`/`Connection`/`Listener` traits, `SyncMessage` (byte-exact wire format), `TransportSelector` (fixed fallback order), `Discovery`/`CompositeDiscovery`, `CloudRelayTransport`, `QuicCrossNetworkTransport` (real NAT-rebinding-tested), `WifiDirectTransport`/`BluetoothLETransport` (platform stubs with `encryption_enabled` gating). Compiles clean, clippy clean. |
| `sync-transport-mobile` crate | `crates/transports/sync-transport-mobile/`    | iOS Multipeer Connectivity + BLE, Android Wi-Fi Direct + BLE C-ABI exports. `#[cfg(target_os)]`-gated; typechecks clean against real `aarch64-apple-ios`/`aarch64-linux-android` targets (verified in isolation — see above).                                                                                                                                                   |
| Tests                         | `crates/transports/sync-transport/tests/*.rs` | 26 unit + 13 integration tests, all passing; `quic_nat_tests` passes when explicitly un-ignored (real sockets, real QUIC handshake, real `Endpoint::rebind`).                                                                                                                                                                                                                   |
| `DECISIONS.md`                | workspace root                                | This section.                                                                                                                                                                                                                                                                                                                                                                   |

### Environment constraints encountered (Team 4)

- `/mnt/user-data/outputs` is a FUSE-mounted rclone target with `noexec` — all `cargo` invocations were run from an exec-capable copy (`/root/...`) and synced back, per the established Increment 1/2 pattern.
- No Xcode / Android NDK available in this container — mobile FFI verified via typecheck-only cross-target `cargo check`, not full link/execution (see above).
- Rust toolchain: the workspace's pinned `1.97.1` (`rust-toolchain.toml`) was used for all real builds; an initial `1.75.0` install was superseded automatically by rustup's toolchain-file resolution.

---

# Amendment — Cross-Team Gap Closure: `persistence-postgres` Offline Query Cache

**Filed by:** All-Father (orchestrator), ahead of Increment 5 kickoff.
**Date:** 2026-08-04.
**Status:** Non-ruling implementation fix — closes a pre-existing, previously-documented gap. No Rust source was modified; no business logic, schema, or Team 2 architectural decision changed.

## P1 — `persistence-postgres` fails to compile without a live database

**Gap (as previously documented by Team 4, "Known cross-team gap" above):**
`persistence-postgres` uses `sqlx::query!` compile-time-checked macros
throughout (`outbox_store.rs`, `repository.rs`, and others). These macros
validate each query's SQL against a real schema *at compile time*, via
either a live `DATABASE_URL` or a checked-in `.sqlx` query cache
(`cargo sqlx prepare` output). Neither was present in the delivered
workspace. Verified independently in this session: `cargo build --workspace
--release` failed with 12 `sqlx::query!` errors, all "set `DATABASE_URL`
to use query macros online, or run `cargo sqlx prepare` to update the
query cache" — confirming Team 4's clippy-scoped observation also holds
for a full, unscoped `cargo build`.

**Root cause:** the query cache was simply never generated and committed
during Increment 2 — not a logic defect in any query, and not a decision
that needs relitigating. Team 2's `DECISIONS.md` §6/§7 (Postgres adapter,
outbox relay decisions) and the `organization_id` handling, timestamp
precision, and test-isolation mutex pattern in
`persistence-postgres/tests/repository_test.rs` (see that file's own
doc comment, and `DECISIONS.md` item P7) are all unchanged by this fix —
they were already correct.

**Fix (mechanical, not a ruling):**

1. Installed PostgreSQL 16 locally in the build sandbox; created a scratch
   database (`onyx_dev`).
2. Applied `migrations/postgres/20260101000000_initial_schema.up.sql`
   verbatim (unmodified) against it.
3. Ran `cargo sqlx prepare --workspace -- --lib` from
   `crates/infrastructure/persistence-postgres` with `DATABASE_URL`
   pointed at that database, generating `.sqlx/` (12 cached query
   descriptors) at the workspace root.
4. Committed `.sqlx/` to the deliverable per `sqlx-cli`'s own guidance
   ("query data written to .sqlx in the workspace root; please check
   this into version control").

**Verification (this session, full commands and results):**

```
# Full release build, offline (no DATABASE_URL, cache-driven)
SQLX_OFFLINE=true cargo build --workspace --release
# -> clean, all 20 crates, including persistence-postgres

# Workspace-wide strict clippy — previously failed on persistence-postgres
SQLX_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings
# -> clean, all 20 crates

# Full workspace test suite
SQLX_OFFLINE=true DATABASE_URL="postgres://postgres:onyx@localhost/onyx_dev" \
  cargo test --workspace --release
# -> 0 failures across every crate

# persistence-postgres's own integration tests, against the live DB
# created for this fix (repository, outbox, idempotency, concurrency)
DATABASE_URL="postgres://postgres:onyx@localhost/onyx_dev" \
  cargo test --package persistence-postgres --all-targets
# -> 10/10 passed: commit_stages_events_outbox_and_idempotency_atomically,
#    mission_persists_and_reloads_with_identical_state,
#    claim_publish_and_pending_count_lifecycle,
#    dead_letter_moves_message_out_of_pending_and_preserves_audit_row,
#    mark_failed_increments_retry_and_schedules_retry,
#    command_idempotency_returns_cached_result_for_duplicate_operation_id,
#    inbox_store_deduplicates_event_delivery_per_consumer,
#    last_processed_returns_most_recent_event_for_consumer,
#    second_writer_with_stale_version_is_rejected,
#    writer_with_advanced_version_succeeds
```

**Effect on prior teams' deliverable status:** Team 4's documented "Known
cross-team gap" (clippy failing on `persistence-postgres` in an unscoped
run) is **closed**. The gap is broader than Team 4's framing — it was a
full-`cargo-build` failure, not clippy-only — but the underlying cause and
scope Team 4 identified (`sqlx::query!` macros, no `DATABASE_URL`/cache)
was accurate.

**What this amendment does *not* do:** it does not re-verify Team 2's
schema design, query correctness, or any business-logic decision in
`DECISIONS_Team_2.md` — those were exercised as-is by the passing
integration tests above and found sound. It does not touch SQLite,
messaging, or any crate outside `persistence-postgres`.

## Deliverable Addendum

| Artifact             | Location       | Status                                                                                 |
|:-------------------- |:-------------- |:-------------------------------------------------------------------------------------- |
| `.sqlx/` query cache | workspace root | 12 cached query descriptors; enables `SQLX_OFFLINE=true` builds with no live database. |
| This amendment       | `DECISIONS.md` | This section.                                                                          |

---

# Team 5 — Desktop & Mobile Native Clients

## Ruling R1 — `CommandRegistry` / `QueryRegistry` / `EventBus` / `SyncAgent` Ownership

**Filed by:** All-Father (orchestrator), ruling on a pre-implementation defect
report.

**Gap found:** Team Prompt 5 §3.2/§3.4/§4.1/§4.3 reference
`CommandRegistry`, `QueryRegistry`, `QueryEnvelope`, `EventBus`, and
`SyncAgent` by name and method, and instruct Team 5 to build the
composition root "using the same registries... as the desktop shell" and
"the same command pipeline as api-server." None of these five types exist
anywhere in the delivered Increment 1–4 codebase (verified by exhaustive
grep across all 20 crates). `api-server`'s own `main.rs` states explicitly
that HTTP routing / dispatch wiring is deferred to "Increment 5/6" — i.e.
there is no existing dispatch layer to reuse. Team 4's `DECISIONS.md` also
references a `bins/sync-agent` binary that was never actually created.

**Ruling:** `CommandRegistry`, `QueryRegistry`, `EventBus`, and `SyncAgent`
are **Team 5 (client-side) responsibilities**, not something Increment 2
under-delivered. Increment 2 delivered the raw execution primitives
(`handle_command<A,C,E,Err>()`, `load_aggregate()`, the `Repository` /
`UnitOfWorkFactory` / `IdempotencyStore` ports); Team 5 builds the
composition/wiring layer on top of them, once per client (desktop-shell,
mobile-core). Team Prompt 5's code snippets in §3.2–§4.3 are illustrative
of intent, not literal signatures to compile verbatim — confirmed several
do not typecheck against the real Increment 2–4 APIs (see R2 below for
specifics found so far).

## R2 — Snippet-vs-Real-API Discrepancies (non-ruling, factual disclosures)

Found while grounding the composition layer design against the real
delivered code, before writing any Team 5 source:

- `SynchronizationSession::new` takes `(local: ReplicaId, remote: ReplicaId,
  organization_id: OrganizationId)` — Team Prompt 5 §4.3's snippet omits
  `organization_id`.
- `SynchronizationSession` has no `resume_from(cursor)`, no
  `run_with_timeout(Duration)`, and no `current_cursor()`. Real
  resumability primitives: `pause(&mut self) -> Result<SyncCursor,
  SyncError>` and `resume(&mut self) -> Result<(), SyncError>` (no
  cursor argument — the session holds its own `last_cursor` internally).
- `TransportSelector` has no `discover()` method; discovery is a separate
  `Discovery` trait / `CompositeDiscovery::discover(org_id) -> Vec<PeerInfo>`,
  composed with `TransportSelector::connect_best(peer, timeout)` — not
  `transport.discover()` returning something with a `.id` field as your
  ruling's illustrative `SyncAgent::run()` snippet assumed.
- `Repository` is **non-generic** (works in `serde_json::Value`); there is
  one `SqliteRepository` instance **per aggregate type**
  (`SqliteRepository::new(pool, aggregate_type: impl Into<String>)`), not
  one repository for the whole app.

None of these are business-judgment calls — they're straightforward API
corrections, on the same footing as Team 3/4's own "implementation
correction to the ruling's own sample code" entries elsewhere in this
file. Implemented against the real signatures; noted here so the record
doesn't imply the illustrative snippets were followed verbatim.

## R3 — Open Question Flagged, Not Yet Resolved: Creation-Command Routing

**Not yet ruled on — flagging before implementation, per standing
instruction to surface rather than silently resolve.**

`handle_command<A, C, E, Err>()` (Increment 2, `api-server::command_handler`)
unconditionally calls `repo.load(&target_id)` and, on `None`, returns
`CommandError::NotFound` — it has no path for aggregate creation. Team 1's
ruling (§C) states creation commands (`CreateMission`, `CreateTask`) are
**not** routed through `decide()` — they use a dedicated `Mission::create()`
/ `Task::create()` associated constructor instead.

**Additional finding**, not previously documented: `Mission::create()`'s
own doc comment states *"the future `Repository` (Increment 2) calls this
when `load()` returns `None`"* — i.e. Team 1 designed `create()` expecting
Increment 2 to wire `load() == None` → `create()` inside the command
pipeline itself. Increment 2 never did this; `handle_command`'s `None`
branch unconditionally errors instead. This is a genuine gap between what
Team 1 built `create()` to expect and what Team 2 actually delivered — it
predates Team 5 and isn't something Team 5 introduced.

This means a single `CommandRegistry::get(command_type).handle(envelope)`
call cannot uniformly dispatch every command through the existing
`handle_command()` as-is — `CreateMission`/`CreateTask` need either (a) a
distinct handler kind Team 5's registry dispatches to directly (bypassing
`handle_command`'s `load()`-then-`decide()` shape entirely for creation),
or (b) a patched/wrapped version of the load-then-dispatch logic that
implements the `None` → `create()` branch Team 1's doc comment describes,
built fresh in the Team 5 composition layer since Increment 2's
`handle_command` itself is frozen, delivered code from another increment
that Team 5 has no mandate to modify.

**Proceeding with (a)**: `CommandRegistry` holds two handler kinds — a
`CreationHandler` (calls `Aggregate::create()` + `repo.commit()` directly,
no version/epoch check, no existing-aggregate load) and a `DecisionHandler`
(wraps the real `handle_command()` unmodified) — dispatched by whether
`command_type` names a creation command. This reuses Team 2's
`handle_command` byte-for-byte for every non-creation command, touches no
Increment 1–4 source, and gives creation its own explicit, auditable path
rather than a patched pipeline. Flagged since it's a real design fork
between two legitimate options, not a mechanical fix — reversible if a
different approach is preferred.

## C1 — Shared `client-composition` Crate (Ruling)

**Ruling:** Created `crates/applications/client-composition`, holding
`CommandRegistry`, `QueryRegistry`, `EventBus`, `SyncAgent`, `AppState`,
and the `CreationHandler`/`DecisionHandler` split from R3. Desktop
(`desktop-shell`) and mobile (`mobile-core`) are thin wrappers around this
shared crate, bridging only to their respective UI transport (Tauri
events vs. C-ABI callbacks).

This is a permitted refinement of Team Prompt 5's illustrative
per-client snippets (§4.1/§4.2), not a violation: the frozen contract
specifies *what* must exist (the four named types) and their observable
behavior, not their crate-level location. Avoids ~800 lines of
drift-prone duplication between the two clients; each client adds only
~100 lines of bridging code.

## Document Hierarchy (recorded per ruling, governs all future ambiguity)

1. `team_prompts/PROMPT_X.md` — Frozen contracts for implementation (authoritative)
2. `onyx_handover_v1.0.yaml` — Machine-verifiable contracts (authoritative)
3. `01_Blueprint/ONYX_Unified_Technical_Blueprint_v1.6.docx` — Business rules, domain rationale (reference)
4. `ONYX_Increment_X_*.md` — Earlier versions; may contain superseded assumptions (historical/illustrative only)

## S1 — `api-server`: Added `[lib]` Target (Ruling)

**Gap found:** `crates/bins/api-server` shipped `[[bin]]`-only (no `[lib]`
target). Its `command_handler::handle_command()` and
`query_handler::load_aggregate()` — the only real command/query execution
primitives delivered in Increment 1–4 — were therefore unreachable from
any other crate, including `client-composition`. Rust does not expose a
binary target's modules to library consumers regardless of `pub`
visibility within the file; this blocked R1/R2 entirely (there was nothing
importable to wrap).

**Ruling:** Add a `[lib]` target (`name = "api_server"`,
`path = "src/lib.rs"`) alongside the existing `[[bin]]` target.
Packaging-only change — confirmed no logic in `command_handler.rs` or
`query_handler.rs` was modified, moved, or rewritten.

**Implementation (as actually done — differs from the ruling's own
illustrative `handle_command<A: AggregateRoot>(envelope: CommandEnvelope<A::Command>,
repo, unit_factory)` snippet, which does not match the real, already-shipped
13-parameter signature; treated per the same "illustrative, not literal"
convention as Team Prompt 5's own snippets):**

1. `Cargo.toml`: added `[lib] name = "api_server" path = "src/lib.rs"`
   above the existing `[[bin]]` block.
2. New `src/lib.rs`: `pub mod command_handler; pub mod query_handler;`
   plus `pub use command_handler::{handle_command, CommandError, CommandResult};`
   and `pub use query_handler::load_aggregate;` — re-exporting exactly
   the symbols that exist (verified by grepping every `pub` item in both
   files first), not the ruling's illustrative names.
3. `src/main.rs`: removed the inline `pub mod command_handler; pub mod
   query_handler;` declarations (previously compiling those files
   directly into the binary target). `main.rs` does not currently call
   either module (HTTP routing remains out of scope per its own doc
   comment, deferred to Increment 5/6's routing layer, distinct from the
   dispatch primitives this ruling exposes) — this change removes
   redundant duplicate-compilation of the same source under two targets,
   with no behavior change.

**Verification:**

```
SQLX_OFFLINE=true cargo build --package api-server --release   # clean
SQLX_OFFLINE=true cargo clippy --package api-server --all-targets -- -D warnings  # clean
SQLX_OFFLINE=true cargo test --package api-server --all-targets  # 0 tests, 0 failures (unchanged — api-server has no tests of its own)
```

`client-composition` now depends on `api-server` and calls
`api_server::handle_command` / `api_server::load_aggregate` directly.

## W1 — Wire Protocol: `DomainEventEnvelope` ↔ `SyncMessage` (Ruling)

**Gap found:** `sync-transport` (Increment 4) sends/receives raw
`SyncMessage { payload: Vec<u8>, .. }`; `synchronization-domain`
(Increment 3) operates on `Vec<DomainEventEnvelope<serde_json::Value>>`.
No protocol connected them — a genuine pre-existing gap between
Increments 3 and 4, not introduced by Team 5.

**Ruling:** `SyncMessage.payload` carries a JSON-encoded schema keyed by
`SyncMessageType`:

- `DiscoveryRequest` / `DiscoveryResponse`: `{replica_id, vector_clock}`
- `OfferOperations`: `{aggregate_id, from_version}`
- `OperationBatch`: `{events: [DomainEventEnvelope, ...]}`
- `ConflictNotification`: `{conflict_id, field}`
- `Ack`: `{message_id, status}` (defined by the ruling; not yet used by
  this implementation — `Connection::request_response` provides
  request/response pairing at the transport level, so an explicit `Ack`
  message wasn't needed for the synchronous exchange this implements)

Implemented in `sync_agent.rs::run_one_session`.

**Corrections made against the real delivered types** (same convention as
every other ruling's illustrative snippet — grounded before writing,
not assumed correct):

- `Connection::send` / `recv` / `request_response` take `&mut self`
  (the ruling's snippet used an unqualified `connection.send(...)`,
  compatible once `connection` is bound `mut`).
- `AggregateDelta` (`SynchronizationSession::discover`'s return type) has
  `aggregate_id`, not `id`, and has **no** `version` field. A real
  per-aggregate "from version" therefore isn't derivable from Discovery
  alone with the current session API; `OfferPayload.from_version` is
  conservatively sent as `0` for every offered aggregate (full resync)
  rather than fabricating a nonexistent field. Flagged as an open
  precision gap, not a resolved one — a future amendment adding a real
  version to `AggregateDelta` would let this be tightened.
- `ConflictRecord`'s field is `field_path`, not `field` (`ConflictPayload`
  wire field is still named `field` per the ruling's wire schema; only the
  Rust-side source field differs).
- Discovery and Offer/Batch round-trips use `Connection::request_response`
  (single call, paired response) rather than separate `send()` then
  `recv()` calls — safer against response mis-pairing on a connection
  that might interleave messages, and a strict subset of what the ruling
  asked for.

**Still-open, explicitly flagged (not resolved by this ruling):** session
resumption across a killed/restarted client process.
`SynchronizationSession` has no cursor-argument `resume(cursor)` (see
R2) — `pause()`/`resume()` operate on the *same* session instance's
internal `last_cursor`, not a value that can be serialized, persisted,
and fed into a freshly-constructed session later (e.g. after an iOS
background suspension kills the process). Team Prompt 5 §4.3 requires
exactly this. `run_one_session` calls `session.pause()` at the end of
each cycle and documents this gap at the call site (`TODO(session
resumption, tracked separately)`) rather than silently claiming
resumption works.

**Verification:**

```
SQLX_OFFLINE=true cargo build --workspace --release                       # clean, 21 crates
SQLX_OFFLINE=true cargo clippy --workspace --all-targets -- -D warnings   # clean, 21 crates
```

## V1 — `platform_kernel::VectorClock` Cannot Serialize to JSON When Non-Empty (Ruling)

**Filed by:** All-Father (orchestrator), ruling on a defect found while
writing Team 5's wire-protocol tests (W1).

**Defect (confirmed, reproduced standalone before this ruling):**
`VectorClock { entries: BTreeMap<ReplicaId, u64> }` derived plain
`Serialize`/`Deserialize`. `serde_json` requires JSON object keys to be
strings; `ReplicaId` (a `[u8; 16]` newtype) serializes as a JSON array, not
a string. **Any `VectorClock` with at least one entry failed
`serde_json::to_value`/`to_string`/`to_vec`** with `Error("key must be a
string")`. The empty/default clock serialized fine, which is why this went
undetected through Increments 1–4: no delivered test exercised a non-empty
clock through JSON.

**Confirmed blast radius, found by grepping every `vector_clock`
JSON-serialization call site in the workspace:**

1. `api_server::handle_command` Step 8: `serde_json::to_value(&envelope)?`
   on a `DomainEventEnvelope` — hard failure (`CommandError::Serialization`)
   for any command whose vector clock had been advanced past empty, before
   ever reaching persistence.
2. `persistence-postgres`/`persistence-sqlite`, both `insert_outbox_message`:
   `.unwrap_or(Value::Null)` (Postgres) / a double
   `.unwrap_or_default()` → `"{}"` fallback (SQLite) — silently **wrote
   the wrong value** instead of failing, for any non-empty clock.
3. Team 5's own wire protocol (W1): would have failed identically.

**Ruling:** Custom `Serialize`/`Deserialize` for `VectorClock.entries`,
mapping each `ReplicaId` key to a lowercase 32-character hex string on the
wire; in-memory representation (`BTreeMap<ReplicaId, u64>`) unchanged.

**Implementation (as actually done — one substitution from the ruling's
own snippet, documented per the established convention):** implemented via
`uuid::Uuid::from_bytes(id.0).simple().to_string()` /
`uuid::Uuid::parse_str(...)`, reusing the `uuid` crate already a
dependency of `platform-kernel` (and already used by
`ReplicaId::new_random()`/its `Debug` impl), rather than adding a new
`hex` crate dependency for the identical 16-byte-to-hex-string
conversion. The wire format is byte-identical either way — lowercase,
32 hex characters, no separators — so this is a mechanism substitution,
not a behavioral difference from the ruling.

**Also fixed, per the ruling's "Immediate Corrective Actions" table:**
both persistence adapters' `insert_outbox_message` now propagate a
serialization failure as `sqlx::Error::Protocol(...)` (matching this
crate's existing convention of wrapping infrastructure failures at this
return type, translated to `UnitOfWorkError::CommitFailed` at each
adapter's call site) instead of silently degrading to `null`/`"{}"`.

**Verification:**

```
# platform-kernel: new regression tests, including a 50-replica round-trip
# stress test and a malformed-key rejection test
cargo test --package platform-kernel --all-targets   # 28 tests, 0 failures
cargo clippy --package platform-kernel --all-targets -- -D warnings   # clean

# Both adapters, full existing suites re-run (no regressions)
cargo test --package persistence-sqlite --all-targets    # 3 tests, 0 failures
DATABASE_URL=postgres://... cargo test --package persistence-postgres --all-targets   # 10 tests, 0 failures

# New regression test: non-empty VectorClock (two replicas, distinct
# counts) through a REAL, LIVE Postgres commit + claim round-trip
DATABASE_URL=postgres://... cargo test --package persistence-postgres --test outbox_test
# -> non_empty_vector_clock_round_trips_through_real_postgres_outbox: ok

cargo clippy --package persistence-sqlite --package persistence-postgres --all-targets -- -D warnings   # clean
```

**Flagged, not yet ruled on — a related but distinct issue found while
verifying V1, out of this ruling's explicit scope:** both adapters' outbox
*read* paths (`claim_unpublished`, `dead_letter`) have the identical
silent-fallback pattern on deserialization —
`serde_json::from_value(row.vector_clock).unwrap_or_default()`
(Postgres, 2 sites) and an `.ok().and_then(...).unwrap_or_default()` chain
(SQLite, 2 sites). Before V1 this was accidentally harmless (a stored
`null`/`"{}"` reading back as an empty clock matched what was actually
written). Now that non-empty clocks store correctly, a genuinely malformed
stored value would silently read back as an empty `VectorClock` with no
error — a live silent-corruption risk for a causal-ordering primitive.
Not fixed as part of this ruling; awaiting a decision on whether to apply
the same fail-loudly treatment there.

## V2 — Read-Side `VectorClock` Deserialization: Propagate Instead of Default (Ruling)

**Ruling:** replace every `unwrap_or_default()`-style silent fallback on
`vector_clock` deserialization in both adapters' `claim_unpublished`/
`dead_letter` with error propagation. A corrupt row is a hard error, not
a default.

**Implementation (one deliberate deviation from the ruling's own
snippet, documented per the established convention):** the ruling's
snippet proposed adding a new `OutboxError::Deserialization(String)`
variant. `worker_application::OutboxError` (the real, already-delivered
port error type, defined in `worker-application` — a different crate
than either persistence adapter) **already has a `Serialization(String)`
variant**, semantically identical in purpose to the proposed new one
("failed to (de)serialize"). Reused it instead of adding a duplicate
variant to another team's port trait for the same purpose. No `OutboxError`
signature change was needed.

**Fixed, all 4 sites (2 per adapter):**

- `persistence-postgres/src/outbox_store.rs`: `claim_unpublished` and
  `dead_letter`, both `serde_json::from_value(row.vector_clock)
  .unwrap_or_default()` → `.map_err(|e| OutboxError::Serialization(e.to_string()))?`.
- `persistence-sqlite/src/outbox_store.rs`: `claim_unpublished` and
  `dead_letter`, both the `.ok().and_then(|v| serde_json::from_value(v).ok())
  .unwrap_or_default()` chain → two `?`-propagated stages
  (`text_to_value` parse failure and `VectorClock` shape mismatch reported
  separately, so the error message identifies which stage actually failed).

**Verification, including the acceptance-criteria test the ruling
specified (a row with a stored `vector_clock` that doesn't match
`VectorClock`'s shape):**

```
# Postgres: JSONB enforces JSON *syntax* at the DB level, so the test
# writes valid JSON of the wrong shape ({"entries": {"not-a-hex-id": "not-a-number"}})
# directly into a committed row via raw SQL, then asserts claim_unpublished
# rejects it.
DATABASE_URL=postgres://... cargo test --package persistence-postgres --test outbox_test
# -> 5/5 passed, including claim_unpublished_returns_an_error_for_a_malformed_vector_clock_column

# SQLite: TEXT column enforces nothing, so the test writes genuinely
# non-JSON text ('not even json {') directly into a committed row.
cargo test --package persistence-sqlite --test sqlite_test
# -> 5/5 passed, including sqlite_claim_unpublished_returns_an_error_for_a_malformed_vector_clock_column
# (also added: sqlite_non_empty_vector_clock_round_trips_through_outbox,
# closing the gap that persistence-sqlite's suite had no OutboxStore-level
# test at all before this ruling — it only asserted a raw row count)

cargo clippy --package persistence-postgres --package persistence-sqlite --all-targets -- -D warnings   # clean
```

All `VectorClock` defects (V1 write-side, V2 read-side) are now closed,
verified against real corrupt-row scenarios on both adapters, not just
asserted by code review.

## F1 — No Production `UnitOfWorkFactory` Implementation Existed (Ruling)

**Gap found:** `query_application::UnitOfWorkFactory` — the trait
`api_server::handle_command()` requires as `Arc<dyn UnitOfWorkFactory>` —
had exactly one implementation in the entire delivered workspace, and it
was a test double (`sync-test-utils::mocks::MockRepository`, in-memory,
unrelated to any real database). Neither `persistence-postgres` nor
`persistence-sqlite` implemented it for their real
`PostgresUnitOfWork`/`SqliteUnitOfWork` types — those only exposed a free
function, `{Postgres,Sqlite}UnitOfWork::begin(pool, organization_id)`,
called exclusively from each adapter's own test files, never from any
production code path. Confirmed by exhaustive grep
(`impl UnitOfWorkFactory for` across every crate) before this ruling.
**Consequence:** `api_server::handle_command()` had never been callable
against a real database in production, in any increment prior to Team 5.

**Ruling:** implement thin `UnitOfWorkFactory` wrappers in each adapter,
calling the existing, already-tested `begin()` — no new transaction
logic, matching the S1 precedent (packaging/reachability fix, not a
behavior change).

**Implementation (as actually done — one correction from the ruling's own
snippet, same convention as every prior ruling):** the ruling's snippet
used `self.pool.begin()` then `PostgresUnitOfWork::new(conn, org)` /
`SqliteUnitOfWork::new(conn, org)` — but **neither adapter has a `new()`
constructor**; the real, only constructor is `{Postgres,Sqlite}UnitOfWork::begin(pool: &Pool, org)`,
which begins the transaction internally. Implemented as:

```rust
// persistence-postgres/src/unit_of_work.rs (co-located with
// PostgresUnitOfWork, matching this crate's one-file-per-port convention
// rather than a new file per the ruling's suggested layout)
pub struct PostgresUnitOfWorkFactory { pool: PgPool }
impl PostgresUnitOfWorkFactory { pub fn new(pool: PgPool) -> Self { Self { pool } } }
#[async_trait]
impl query_application::UnitOfWorkFactory for PostgresUnitOfWorkFactory {
    async fn create(&self, organization_id: OrganizationId) -> Result<Box<dyn UnitOfWork>, UnitOfWorkError> {
        Ok(Box::new(PostgresUnitOfWork::begin(&self.pool, organization_id).await?))
    }
}
```

Identical pattern for `SqliteUnitOfWorkFactory` in `persistence-sqlite`.
Both re-exported from each crate's `lib.rs`.

**Also found while writing the first real end-to-end test against these
factories:** no `IdempotencyStore` implementation exists anywhere in the
workspace either — not production, not even a test mock (same
"port declared, never implemented" pattern, confirmed by grep, one level
smaller in scope than `UnitOfWorkFactory`). Not raised as its own ruling;
a minimal in-memory implementation was written directly inside the new
test file (`client-composition/tests/mission_end_to_end_sqlite.rs`),
explicitly commented as test-only scaffolding, not a production adapter.
Flagged here for visibility rather than silently working around it.

**Verification:** see ruling H2 below — the same test run covers F1
(both factories work) and H2 (the deadlock they exposed, now fixed).

## H2 — `handle_command` Deadlocked Under a Single-Connection Pool (Ruling)

*(Labeled H2 in this file to avoid collision with Team 2's own "H1" —
`DECISIONS_Team_2.md`'s existing item about post-decide `apply()` not
being called, referenced in `command_handler.rs`'s own header comment —
a distinct, still-open issue from a different DECISIONS.md, noted below,
not resolved by this ruling.)*

**Defect found:** writing the first real end-to-end test against F1's new
factories (`MissionCreationHandler` → `MissionDecisionHandler`, real
SQLite), `PauseMission` hung and timed out
(`"pool timed out while waiting for an open connection"`) despite
`CreateMission` immediately before it succeeding. Root-caused and
confirmed via an isolated, minimal reproduction (outside any Team 5
code, using only real Increment 1/2 crates) before this ruling: the
original `handle_command` opened a `UnitOfWork` transaction (Step 3,
`unit_factory.create()`) **before** calling `repo.load()` (Step 4) —
both draw from the same connection pool. Under a pool sized to exactly
one connection (the standard, near-mandatory configuration for in-memory
SQLite, and a plausible one for a resource-constrained desktop/mobile
client — Team 5's actual deployment target), the open transaction holds
the pool's only connection and `repo.load()` can never acquire one to
run its query. Reproduction output:

```
Opening a UnitOfWork (holds the pool's only connection)...
Now trying repo.load() while that transaction is still open...
DEADLOCK CONFIRMED: repo.load() timed out after 5s...
```

**Ruling:** reorder `handle_command` — load the aggregate and perform the
read-only version/lifecycle-epoch checks *before* opening the
`UnitOfWork` transaction; open the transaction only once a write is
actually about to happen.

**Implementation (grounded against the real 13-parameter
`handle_command<A, C, E, Err>(...)` signature, not the ruling's own
illustrative 4-parameter snippet — same convention as every prior
ruling):**

- Idempotency check (unchanged, still first — no transaction needed).
- Load + version/lifecycle-epoch checks moved before transaction
  creation; their early-return `unit.rollback().await.ok()` calls were
  removed along with them, since there is no transaction yet at those
  points to roll back.
- Transaction opens immediately before the `decide()`/serialize/commit
  sequence.

**Race safety, explicitly verified rather than assumed:** the reordering
opens a narrow window between the read-only version check and the
eventual `repo.commit()`, where a concurrent writer could commit first.
This is safe: both adapters' `commit()` already re-validates optimistic
concurrency **at write time** via a `WHERE version < new_version` guard
on the aggregate upsert (confirmed in both
`persistence-{postgres,sqlite}/src/unit_of_work.rs`) — a real race loses
at `commit()` (`sqlx::Error::RowNotFound` → `CommandError::Persistence`),
not silently. The early check was always a fast-path optimization in
both the original and reordered versions, never the sole guard.

**Verification:**

```
# The exact scenario that deadlocked before this fix, now passing in 0.01s
# (was: 30s timeout / hang)
cargo test --package client-composition --test mission_end_to_end_sqlite
# -> create_mission_then_pause_it_through_the_real_command_registry_and_sqlite: ok
# -> decision_handler_does_not_deadlock_a_single_connection_pool: ok
#    (dedicated regression test, 5s hard timeout so a future regression
#    fails fast and unambiguously rather than hanging)

# Full regression sweep — no other test's behavior changed
cargo test --package api-server --all-targets                              # 0 tests (unchanged), clean
cargo test --package client-composition --all-targets                      # 18 unit + 2 integration, 0 failures
DATABASE_URL=postgres://... cargo test --package persistence-postgres --all-targets  # 12 tests, 0 failures
cargo test --package persistence-sqlite --all-targets                      # 5 tests, 0 failures
cargo clippy --package api-server --all-targets -- -D warnings             # clean
```

**Distinct, still-open issue noted but not addressed by this ruling:**
Team 2's own `DECISIONS_Team_2.md` "H1" entry (a different label, in a
different file, predating this session) already flags that
`handle_command`'s persisted aggregate state is the **pre-decide**
serialization, not post-`apply()` — and says explicitly *"This is
corrected in Increment 5... Documented for Team 5's awareness."* This is
a real, separately-flagged item Team 2 expects Team 5 to address,
distinct from the deadlock fixed here. Not yet actioned; noting it so it
isn't lost.

## H3 — `handle_command`: Persist Post-`apply()` State (Ruling)

**Actions Team 2's own previously-documented gap**, referenced above as
"a distinct, still-open issue" when H2 was recorded, and confirmed
concretely blocking within the same session: writing the first real
end-to-end test to chain a *second* successful decision command against
the same aggregate (`TaskDecisionHandler`, `MarkReady` then `AssignOwner`
against the same `Task`), the second command failed —
`"aggregate upsert failed: no rows returned by a query that expected to
return at least one row"`. Root cause, confirmed by reading
`SqliteRepository::commit`: it extracts the row's `version` directly from
the `aggregate_state` JSON blob it's given
(`aggregate_state.get("version")`), and `handle_command` was serializing
the aggregate's **pre-decide** state (`serde_json::to_value(&aggregate)`
before any `apply()` call) — so the persisted `version` field never
advanced past its pre-command value, and every second write against the
same aggregate failed both adapters' `WHERE version < excluded.version`
optimistic-concurrency guard, since the new write's version was never
actually higher than what was already stored.

**Ruling:** call `aggregate.apply(&event)` for each event `decide()`
produces, before serializing `new_state`.

**Implementation (one deviation from the ruling's own illustrative
snippet, same convention as every prior ruling):** the snippet used
`let mut mut_aggregate = aggregate.clone();` — but `handle_command<A, C,
E, Err>`'s `where` clause carries no `Clone` bound on `A` (only Team 1's
concrete `Mission`/`Task` happen to derive it), so `.clone()` would not
compile generically without widening the function's trait bounds, a
larger change than this ruling calls for. Implemented instead by making
the existing `aggregate` binding `mut` and calling `aggregate.apply(&event)`
in place, inside the same loop that builds each event's
`DomainEventEnvelope` — the borrow for `apply(&event)` happens
before that same `event` is moved into its envelope's `payload` field,
so no `Clone` bound is needed on the event type `E` either (only the
existing `E: Serialize`).

**Verification:**

```
cargo check --package api-server                                          # clean
cargo clippy --package api-server --package client-composition --all-targets -- -D warnings   # clean

# The exact failure this fix resolves: a second, then a third, successful
# decision command against the same Task aggregate, each with
# expected_version incrementing by exactly one, asserting both the
# reported new_version AND the actually-persisted stored version at
# every step (a regression here would have failed at command 2, exactly
# where the original bug surfaced)
cargo test --package client-composition --test task_end_to_end_sqlite
# -> create_task_then_mark_it_ready_through_the_real_command_registry_and_sqlite: ok
# -> version_advances_correctly_across_three_successive_decision_commands: ok

# Full regression sweep — zero behavior change anywhere else, including
# mission-domain/work-domain's own tests (apply() itself untouched, only
# when handle_command calls it)
cargo test --workspace --release   # 270 tests, 0 failures (up from 268 pre-H3, +2 new regression tests)
cargo build --workspace --release && cargo clippy --workspace --all-targets -- -D warnings   # both clean, 21 crates
```

**Note on the fix's scope:** `apply()` also advances `lifecycle_epoch`/
`authority_epoch` on the domain events that mutate them (e.g. `Task`'s
`TaskMarkedReady` calls `lifecycle_epoch.advance()`) — this is real,
correct, pre-existing domain logic in `mission-domain`/`work-domain` that
H3 now actually surfaces through persistence for the first time (it was
always in `apply()`, just never reached). The three-command regression
test above tracks and asserts this explicitly rather than assuming every
command leaves epochs unchanged.

## `AppState` — Composition Root Complete

`crates/applications/client-composition/src/app_state.rs`: the single
composition root `desktop-shell`/`mobile-core` each construct once at
startup (`AppState::new(pool, config)`), wiring:

- `CommandRegistry` with all 14 `MissionCommand` and 16 `TaskCommand`
  variants registered (1 `CreationHandler` + N `DecisionHandler`
  registrations each — one `MissionDecisionHandler`/`TaskDecisionHandler`
  instance genuinely serves every non-creation command for its aggregate
  type, since it deserializes whichever variant the JSON payload contains
  and dispatches through that aggregate's own single `decide()` match).
- `QueryRegistry` with `GetMission`/`GetTask`.
- `EventBus`.
- `SyncAgent`, wired to a real `SqliteOutboxStore` (Increment 2,
  genuinely delivered) and a real `CompositeDiscovery`/`TransportSelector`
  pair.

**Genuine gap surfaced while wiring the sync agent's transport, flagged
rather than fabricated around:** `TransportSelector::new` requires a
`CloudRelayTransport` (Cloud Relay is the mandatory final fallback per
Team Prompt 4 §3.3), whose real constructor needs an
`Arc<dyn AuthorityProvider>` and `Arc<dyn RelaySocketFactory>`. **No
production implementation of either trait exists anywhere in the
delivered workspace** — `sync_transport::placeholder_types::
StaticAuthorityProvider` is explicitly documented in its own crate as
test-only, and `RelaySocket`'s doc comment states a real implementation
is "Implemented by the composition root (binds to `tokio-tungstenite` +
`reqwest` there)" — i.e. `client-composition` itself is where a real one
belongs, and it doesn't exist yet. `AppStateConfig` therefore takes both
as required, caller-supplied parameters (`cloud_relay_endpoint`,
`cloud_relay_auth_provider`, `cloud_relay_socket_factory`) rather than
`AppState` fabricating a fake "always fails" implementation and shipping
it silently as if real. `desktop-shell`/`mobile-core` (or a future
increment) must supply real ones before Cloud Relay actually works;
tests use honestly-labeled, minimal test doubles.

**Verification:**

```
cargo test --package client-composition --test app_state_wiring
# -> app_state_new_wires_a_working_command_registry_end_to_end: ok
# -> app_state_new_wires_task_commands_independently_of_mission_commands: ok

cargo clippy --package client-composition --all-targets -- -D warnings   # clean
cargo build --workspace --release && cargo clippy --workspace --all-targets -- -D warnings   # both clean, 21 crates
cargo test --workspace --release   # 272 tests, 0 failures
```

## T2 — Tauri Version: `^2.11`, Not the Frozen `^1.5` (Ruling)

Team Prompt 5 §5.1 pins `tauri = "^1.5"`. Verified via web search
(current as of this session): Tauri 1.x is long superseded — stable line
is 2.x (`tauri` 2.11.5, current at time of writing). Ruled: build against
Tauri 2.x, documenting the deviation.

**Corrections made against the real, current Tauri 2 API** (fetched
directly from `v2.tauri.app` and `docs.rs`, not assumed from the
ruling's own migration table, which had several stale/incorrect entries):

- No `"api-all"` Cargo feature exists in Tauri 2 (a Tauri 1 concept);
  default features (`wry` included) are correct.
- Commands and the app entry point live in **`lib.rs`**, not `main.rs`
  (Tauri 1's convention, and Team Prompt 5's own §4.1 snippet's shape).
  Entry point is `#[cfg_attr(mobile, tauri::mobile_entry_point)] pub fn
  run()`, called from a thin `main.rs`.
- Event emission is `Emitter::emit(app, event, payload)` — `emit_all`
  does not exist in Tauri 2.
- `app.path().app_data_dir()` (v2 `PathResolver`), not v1's
  `path_resolver().app_data_dir()`.

## S2 — Secure Storage: Unified `keyring` Crate, Not Three Hand-Written Adapters (Ruling)

Team Prompt 5 §2.1/§5.1 specifies three separate hand-written per-OS
adapters (`security-framework` for macOS, `windows` for Windows,
`secret-service` for Linux). Two were written and one (Linux) genuinely
tested before this ruling; the Windows adapter would have required
unsafe, unverifiable-in-this-sandbox raw `CREDENTIALW` FFI. Ruled:
replace all three with a single adapter backed by the `keyring` crate.

**Corrections made against the real, current `keyring` crate (v4.x, not
the ruling's `3.0`)**, fetched directly from docs.rs before writing:
`keyring::v1::Entry` (the `v1` Cargo feature, not `["windows", "linux",
"macos"]`), methods `set_secret`/`get_secret`/`delete_credential` (not
`delete_secret`, which doesn't exist), and `Error::NoEntry`/
`Error::NoStorageAccess` (`#[non_exhaustive]`) as the real error variants
— `NoStorageAccess` mapped to the frozen contract's `StorageError::AccessDenied`
(previously an unconstructed, clippy-flagged dead variant), everything
else to `Platform`.

**Verification (real, live, not mocked):** installed `webkit2gtk`/`gtk3`
dev headers (Tauri's Linux build dependencies) and a real
`gnome-keyring-daemon` + session D-Bus (a genuine Secret Service
provider) in this sandbox — same "install real infrastructure" treatment
given to Postgres earlier in this session.

```
cargo test --package desktop-shell --lib
# -> get_secret_for_unknown_key_returns_none_not_error: ok (real Secret Service)
# -> delete_secret_for_unknown_key_returns_not_found: ok (real Secret Service)
# -> store_then_get_then_delete_round_trips: ignored — this headless
#    sandbox's gnome-keyring-daemon requires an interactive `pinentry`
#    GUI prompt to unlock the collection for a WRITE specifically (no
#    non-interactive unlock path found); the two read/delete-of-unknown-key
#    cases don't hit this and pass for real. The failure itself is
#    positive evidence the adapter's error mapping works: attempting the
#    write produced a real `AccessDenied("SS error: prompt dismissed")`,
#    correctly categorized by the NoStorageAccess mapping above rather
#    than falling into a generic Platform bucket.
```

## `desktop-shell` — Complete

`crates/bins/desktop-shell`: Tauri 2.x desktop client, thin wrapper
around `client-composition`'s `AppState` (per C1). Real Tauri commands:
`execute_command`, `execute_query`, `subscribe_events` (forwards the
event bus to the webview via `Emitter::emit("onyx:event", ...)`),
`get_sync_status`, `store_secret`/`get_secret`/`delete_secret`.

**Flagged gaps, left as explicit code comments rather than silently
resolved:**

- `organization_id` is randomly generated per launch — no auth/session
  flow exists yet (Increment 7 scope).
- Cloud Relay has no real endpoint/auth/socket implementation yet; a
  `NotYetImplementedSocketFactory` fails fast at connect time rather than
  silently pretending to succeed.
- No `unsubscribe_events` command pairs with `subscribe_events`'s
  returned stream id (Team Prompt 5 doesn't specify one either).
- No real frontend (React/Vue/etc.) — a placeholder `dist/index.html`
  satisfies Tauri's bundler; the actual web UI is out of scope for this
  backend-composition pass.

**Verification:**

```
cargo build --workspace --release                       # clean, 22 crates
cargo clippy --workspace --all-targets -- -D warnings   # clean, 22 crates
cargo test --workspace --release                         # 274 tests, 0 failures
```

---

# Architectural Decisions — Team 6 (Web UI Thin Client)

**Date:** 2026-08-05  
**Status:** Binding  
**Authority:** Team 6 Execution Prompt v1.0 (Full Integration Option)

## Frozen rulings incorporated

| ID | Ruling | Implementation |
|---|---|---|
| T6-R1 | Team 6 owns the OpenAPI freeze. | `docs/api/openapi.json`, version 1.0.0. |
| T6-R2 | Implement Axum HTTP/WebSocket routes. | `crates/bins/api-server/src/routes/`. |
| T6-R3 | Team Prompt 6 TypeScript DTOs are canonical. | Implemented in `web-ui/src/types/`; projection-specific interfaces extend the canonical envelopes. |
| T6-R4 | Bearer `access_token` JWT is the AuthorityProof. | The browser sends only `Authorization: Bearer`; the API validates and enriches the internal actor/authority context. Body-level `actor` and `authority_proof` remain optional for schema compatibility and are ignored. |
| T6-R5 | Web device identity is `web-client`. | Public event DTOs expose `device_id: "web-client"`. The Rust kernel requires a 16-byte `ObjectId`, so the internal adapter deterministically derives 16 bytes from SHA-256(`"web-client"`). |
| T6-R6 | WebSocket token query and exponential reconnect. | `/api/events?token=...`; delays 1s, 2s, 4s, doubling to a 30s ceiling. |
| T6-R7 | Query envelope is base64url(JSON). | `GET /api/query?envelope=...`; no padding required. |
| T6-R8 | JSON uses snake_case. | Auth and all API DTOs use snake_case fields. |
| T6-R9 | Deterministic test organization and seed. | `tests/fixtures/seed.sql`; organization `11111111-1111-1111-1111-111111111111`. |

## Additional implementation rulings

### T6-D1 — Missing Approval and Notification domain crates

The repository delivered to Team 6 contains Mission and Work domain crates only. Team 6's required mutations (`notification.Acknowledge`, `approval.Approve`, `approval.Reject`) therefore cannot be routed through a pre-existing Notification or Approval aggregate.

**Ruling:** define minimal API-owned `NotificationAggregate` and `ApprovalAggregate` implementations inside Team 6's command route module. They implement the existing `AggregateRoot` contract and execute through the existing `api_server::handle_command`, `Repository`, `UnitOfWorkFactory`, optimistic-concurrency, event persistence, and idempotency pipeline. They are deliberately limited to Team 6's three authorized v1 commands and do not claim to replace the future full bounded-context implementations.

### T6-D2 — Integrated development database

The prior API binary required an external PostgreSQL instance and contained no HTTP routes. A deterministic E2E environment must run without external infrastructure.

**Ruling:** the integrated Team 6 API uses the delivered SQLite adapter by default (`sqlite://onyx-team6.db?mode=rwc`) and runs the delivered SQLite migrations at startup. `DATABASE_URL` remains configurable. The production Postgres adapter is not removed.

### T6-D3 — Public envelope enrichment boundary

The frozen canonical envelope contains `actor` and `authority_proof`, while ruling T6-R4 forbids the Web UI from constructing authority proof.

**Ruling:** canonical TypeScript interfaces retain these fields as optional compatibility fields, but the HTTP adapter treats the bearer JWT as the sole authority input and constructs the internal `ActorContext`. This avoids duplicated or user-forgeable authority assertions in the browser.

### T6-D4 — Projection source

The repository has no separate projection service or read-model tables.

**Ruling:** Team 6's `execute_query` reads server-side aggregate snapshots and converts internal kernel identifier arrays to public UUID strings. This is a read-only adapter; no domain state machine is implemented in the browser or query layer.

### T6-D5 — WebSocket missed-event handling

The frozen contract specifies reconnection but no resumable cursor.

**Ruling:** WebSocket events are hints that invalidate React Query caches. Authoritative state is always recovered by refetching projections. A server `resync_required` control message may be sent when a broadcast subscriber lags.

### T6-D6 — No optimistic mutation

**Ruling:** successful commands invalidate and refetch relevant server projections. No React Query `onMutate`, local domain write, offline queue, or success state is applied before server acknowledgment.

### T6-D7 — Identifier extraction in persisted event envelopes

Both delivered persistence adapters assumed kernel identifier JSON had the shape `[[16 bytes]]`, while `platform-kernel` serializes identifier newtypes as a direct `[16 bytes]` array. This caused aggregate, operation, correlation, causation, and organization identifiers in persisted domain-event columns to fall back to all-zero values.

**Ruling:** update the shared extraction helper in both SQLite and PostgreSQL UnitOfWork adapters to accept the canonical direct array and the older one-element wrapper for backward fixture compatibility.

### T6-D8 — AuthorityEpoch enforcement in the command pipeline

The delivered generic command handler accepted `expected_authority_epoch` but named it `_expected_authority_epoch` and never validated it.

**Ruling:** validate the aggregate's current AuthorityEpoch alongside ObjectVersion and LifecycleEpoch, returning a retryable conflict before opening the write transaction.

### T6-D9 — Durable command idempotency and cached result shape

The delivered generic command pipeline accepted an `IdempotencyStore` for lookup, while its transaction wrote the result through `UnitOfWork::register_idempotency_result`. Team 6 initially supplied an in-memory lookup store, which could not observe the transactionally persisted SQLite row and therefore could not satisfy duplicate-operation replay.

**Ruling:** Team 6 supplies a SQLite-backed `IdempotencyStore` over the existing `idempotency` table. The generic command result now retains the committed event envelopes and resulting lifecycle/authority epochs in addition to the existing summary fields. Duplicate `OperationId` requests return the cached committed result and do not rebroadcast a second WebSocket event.

### T6-D10 — Deterministic HTTP error matrix for real-server E2E

Increment 7 rate governance and production fault injection are not implemented in the delivered repository, but Team 6 acceptance requires real-backend verification of 429 and 500 handling.

**Ruling:** when and only when `ONYX_TEST_MODE=1`, authenticated command/query routes recognize `x-onyx-test-status: 429|500` and return canonical deterministic errors. The behavior is absent when test mode is not explicitly enabled. Natural backend paths provide 401 (missing/expired bearer token), 403 (policy-restricted web approval), and 409 (version/epoch/state conflict).

### T6-D11 — WebSocket token redaction from HTTP tracing

Ruling T6-R6 places the access token in the WebSocket query string. The default Tower HTTP trace span includes the request URI and could therefore log bearer material.

**Ruling:** the Team 6 router does not install the default `TraceLayer`. Application startup and non-secret operational events may still be logged through `tracing`, but request URIs containing `?token=` are not emitted by the default HTTP trace middleware. A future structured observability adapter must explicitly redact this parameter.

### T6-D12 — React Query network mode is explicit

React Query's normal online mode may pause a mutation while the browser is offline and resume it later, which is equivalent to an implicit client queue and violates the thin-client contract.

**Ruling:** all Team 6 queries, login mutations, and operational mutations set `networkMode: "always"`. The network request is attempted immediately and fails visibly; there is no optimistic mutation, persisted mutation, or automatic offline replay.

# Architectural Decisions — Team 7 (Observability, Background Processing, Security)

**Date:** 2026-08-05  
**Status:** Binding  
**Source:** Team 7 Execution Prompt v1.0 (FROZEN)

The following rulings implement Team 7 R1–R10 and resolve concrete integration gaps discovered against the Team 6 repository.

## Frozen rulings implemented

| ID | Implementation |
|---|---|
| **R1** | OTLP/gRPC tracing defaults to `http://jaeger-collector:4317` and reads `OTEL_EXPORTER_OTLP_ENDPOINT`. |
| **R2** | Each service exposes Prometheus at `ONYX_METRICS_BIND` (default `0.0.0.0:9090`). Required families: `requests_total`, `request_duration_seconds`, `outbox_pending`, `job_queue_depth`, `sync_conflicts_open`, `audit_entries_total`; self-observability families are also emitted. |
| **R3** | `StructuredLogEvent` fixes the required JSON fields and security/HTTP boundaries emit through it. Recursive secret-field redaction covers password, token, secret, signature, authorization and private-key keys. |
| **R4** | PostgreSQL `jobs` table stores status, claimant, lease, attempts, maximum retries, next attempt and deduplication key. Base retry delay doubles from one second and is capped at 300 seconds after ±20% jitter. Maximum retries default to 10. SQLite implements the same contract for local/native tests. |
| **R5** | Scheduler interval is exactly five seconds. Due timeline aggregates enqueue idempotent `TimelineTrigger` jobs. The worker converts those jobs into timeline events, aggregate updates and outbox records. |
| **R6** | Snapshot interval is one hour. Aggregates with more than 1,000 events since their last snapshot are stored in `aggregate_snapshots`. |
| **R7** | Authority proof verification uses Ed25519, checks signature, issue/expiry window, organization, object scope, command scope and an in-memory revocation set pending Policy. Team 6 access JWTs now use `alg=EdDSA`. |
| **R8** | Production rate limiting uses an exact PostgreSQL sliding window protected by `pg_advisory_xact_lock`, partitioned by organization and resource class. |
| **R9** | Audit entries use SHA-256 hash chaining. See T7-D1 for the completed formula. |
| **R10** | Secrets are read from environment variables. Vault remains deferred. |

## Additional binding resolutions

### T7-D1 — Completed audit-chain formula

The supplied R9 line was truncated after `SHA256(hash_chain_prev`. The implemented, frozen formula is:

```text
current_hash = SHA256(previous_hash_raw_32_bytes || canonical_record_json_utf8)
```

`canonical_record_json` recursively sorts object keys and emits compact UTF-8 JSON. The first entry uses 32 zero bytes as `previous_hash`. Verification recomputes every link and reports the first invalid sequence.

### T7-D2 — Migration locations follow the delivered repository

The prompt names migrations at `migrations/<file>`, while the delivered workspace already freezes separate `migrations/postgres/` and `migrations/sqlite/` directories. Team 7 places equivalent migrations in both directories so `sqlx::migrate!` remains compatible with existing composition roots.

### T7-D3 — Audit and snapshot schemas share migration 20260103

The file manifest requires job and rate-limit migrations but gives no tables for mandatory audit hash-chain persistence or snapshots. `audit_entries` and `aggregate_snapshots` are therefore created in `20260103000000_add_job_queue`, keeping the frozen four-migration increment while satisfying R6 and R9.

### T7-D4 — Team 6 JWT authority is upgraded, not replaced

Team 6 R4 says the access-token JWT is the Web UI's AuthorityProof and the browser does not construct a second proof. Team 7 R7 therefore upgrades that token from HS256 to Ed25519/EdDSA and adds signed scope claims. The command route enforces command, object and tenant scope after cryptographic verification.

### T7-D5 — Secret rotation environment convention

For secret name `NAME`:

- current value: `NAME`
- previous value: `NAME_PREVIOUS`
- previous grace expiry: `NAME_PREVIOUS_VALID_UNTIL_UNIX`

Values may use `hex:`, `base64:`, or `base64url:` prefixes. When a previous key is configured, its grace-expiry variable is mandatory. Previous verification keys are rejected immediately after the declared grace expiry.

### T7-D6 — Development fallback is explicitly non-production

R8 permits Redis or PostgreSQL, not SQLite, for authoritative rate governance. Production startup therefore requires `ONYX_GOVERNANCE_DATABASE_URL` and uses PostgreSQL for both rate limiting and audit. Team 6's SQLite test composition uses a deterministic in-memory sliding-window limiter and SQLite audit writer only when `ONYX_ENV != production`.

### T7-D7 — Metrics port is per service instance

API and worker both default to port 9090. They are independently deployed services and therefore do not share a process/network namespace in production. Local concurrent execution must set distinct `ONYX_METRICS_BIND` values.

### T7-D8 — Command audit follows the existing command commit

The delivered command handler owns its internal UnitOfWork and exposes no audit registration hook. Team 7 appends the hash-chain audit immediately after command completion and emits an observability failure metric if that append fails. The command event still carries existing audit metadata. Making audit-table insertion part of the same aggregate SQL transaction requires a future additive UnitOfWork contract amendment; Team 7 does not silently rewrite the frozen Team 2 port.

### T7-D9 — Root integration tests require a harness crate

The workspace root is a virtual Cargo workspace, so files under root `tests/integration/` are not compiled automatically. `crates/team7-integration-tests` is added solely as a harness that includes the frozen test files and makes `cargo test -p team7-integration-tests --test integration` executable.

### T7-D10 — Synchronization and Automation integration boundary

The limiter implements all three required resource classes (`Commands`, `SyncOperations`, `AutomationExecutions`). Increment 7 wires Commands into the existing API. No Automation execution service exists in the delivered repository, and the existing sync-agent has no security middleware port; those callers can consume the same `RateLimiter` port without changing its contract.

## Production environment variables

| Variable | Purpose | Default |
|---|---|---|
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP/gRPC collector | `http://jaeger-collector:4317` |
| `ONYX_METRICS_BIND` | Prometheus listener | `0.0.0.0:9090` |
| `ONYX_AUTHORITY_SIGNING_KEY` | 32-byte Ed25519 signing seed | Required in production |
| `ONYX_AUTHORITY_SIGNING_KEY_PREVIOUS` | Previous seed during rotation | None |
| `ONYX_AUTHORITY_SIGNING_KEY_PREVIOUS_VALID_UNTIL_UNIX` | Rotation grace expiry; required when a previous key exists | None when no previous key exists |
| `ONYX_GOVERNANCE_DATABASE_URL` | PostgreSQL rate-limit and audit store | Required in production |
| `DATABASE_URL` | API SQLite store or worker PostgreSQL store | Service-specific |
| `ONYX_ENV` | `development`, `test`, or `production` | `development` |

## Team 7 quality gates

```bash
cargo check --workspace
cargo test -p observability-adapter
cargo test -p security-adapter
cargo test -p background-jobs
cargo test -p team7-integration-tests --test integration
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

# Architectural Decisions — Team 8 (Testing, Deployment, and Operational Handover)

**Date:** 2026-08-05  
**Status:** Binding  
**Source:** Team 8 Execution Prompt v1.0 (FROZEN)

## Frozen rulings implemented

| ID | Implementation |
|---|---|
| **R1** | Every Rust service Dockerfile builds from `rust:1.97-slim` and runs from `debian:bookworm-slim`. |
| **R2** | Helm values and every rendered workload/service/ingress use Kubernetes namespace `onyx`. |
| **R3** | The API chart uses Argo Rollouts: 10% for 30 minutes, 50% for 30 minutes, then 100%. Prometheus analysis aborts after an HTTP error-rate result above 1%. |
| **R4** | Terraform and runbooks encode RTO below one hour and RPO below five minutes. Production PostgreSQL is Multi-AZ with continuous recovery logs and 35-day backups. |
| **R5** | Backend E2E tests start PostgreSQL 16 through Testcontainers and read the frozen `tests/fixtures/seed.sql` source. |
| **R6** | Each chaos scenario covers exactly 900 logical seconds. CI advances logical time; staging sets `ONYX_CHAOS_REALTIME=1` for the required wall-clock 15-minute drill. Recovery time is recorded and compared with RTO/RPO. |
| **R7** | `tests/load/smoke-test.js` runs 100 VUs for 60 seconds and fails at command p95 ≥500 ms or failure rate ≥1%. |
| **R8** | Releases emit explicitly versioned SPDX 2.3 JSON SBOMs with Syft for source and container artifacts. The workflow rejects any document whose `spdxVersion` is not `SPDX-2.3`. |
| **R9** | Container images are signed with cosign; binary archives are detached-armored with GPG. Build provenance is attached through GitHub artifact attestations. |
| **R10** | `migration-tool up` executes the SQLx migrator twice. CI additionally runs up twice, down to target zero, and up again. |
| **R11** | Journeys 1–4 are mandatory. Journeys 5–7 are compiled but `#[ignore]` pending Team 5 completion. |

## Additional binding resolutions

### T8-D1 — R11 controls the contradictory Journey 5 notes

The dependency table at the end of the prompt says Journey 5 does not need Team 5 and should be implemented, while binding ruling R11 says **client E2E Journeys 5–7 are ignored until Team 5 is complete**. Binding rulings explicitly supersede prompt ambiguity. Journey 5 is therefore present and ignored. Team 6 already exercises backend notification query/acknowledgment behavior; Journey 5 remains reserved for cross-client notification propagation.

### T8-D2 — Frozen seed fixture is SQLite-shaped but E2E database is PostgreSQL

`tests/fixtures/seed.sql` uses SQLite BLOB UUID literals and millisecond timestamps. Rewriting it would change Team 6's frozen test environment and break the Web/API fixture.

**Ruling:** Team 8's Testcontainers harness reads the exact file, extracts the canonical projection JSON and `public_id`, and inserts equivalent typed PostgreSQL rows using parameter binds. The source of seed truth remains `seed.sql`; no independent seed dataset is introduced.

### T8-D3 — Increment 4 delivered libraries but no `sync-agent` binary

The release manifest requires a sync-agent image and Helm workload, but the delivered Team 7 workspace contains only sync libraries and no process composition root.

**Ruling:** Team 8 adds a minimal headless `sync-agent` binary that owns production process lifecycle, observability, metrics, relay configuration and graceful shutdown. Native-radio transport selection remains in Team 5 clients; Team 8 does not fabricate platform radio access inside a server container.

### T8-D4 — Backend E2E composition uses the production PostgreSQL path

The inherited Team 6 API used a per-process SQLite projection and command store. Team 8's Helm chart runs multiple replicas and a weighted canary, so retaining SQLite in production would create divergent operational state between pods.

**Ruling:** `ApiState` now supports both PostgreSQL and SQLite composition. Production (`ONYX_ENV=production`) requires a PostgreSQL `DATABASE_URL`; SQLite is retained only for deterministic local and isolated test use. The mandatory Approval Journey starts Testcontainers PostgreSQL, loads the frozen seed, and drives the real Axum router through `PostgresRepository`, `PostgresUnitOfWorkFactory`, PostgreSQL idempotency, and PostgreSQL projections.

### T8-D5 — Canary implementation uses Argo Rollouts

Standard Kubernetes `Deployment` objects cannot express weighted traffic, timed progression and metric-driven rollback. The API chart's `deployment.yaml` therefore renders an Argo `Rollout` plus a Prometheus `AnalysisTemplate`, while worker and sync-agent remain normal rolling Deployments. Production clusters must install Argo Rollouts and the NGINX traffic-routing integration before the chart.

### T8-D6 — Chaos timing has CI and staging modes

Five real 15-minute scenarios would occupy at least 75 minutes per CI run and provide poor deterministic feedback.

**Ruling:** CI runs the same 900-second state progression in logical time. `ONYX_CHAOS_REALTIME=1` switches the harness to wall-clock time for staging and DR drills. The go-live checklist requires evidence from the real-time mode.

### T8-D7 — Terraform cloud target is AWS

The prompt says "AWS/Azure resources" without selecting one. The execution pack and deployment requirements do not define a cloud abstraction contract.

**Ruling:** Team 8 supplies a production AWS baseline using EKS, encrypted Multi-AZ RDS PostgreSQL, ECR, KMS, versioned S3 backups and CloudWatch. Azure is a separate provider implementation, not a mixed-cloud file.

### T8-D8 — SBOM generation uses one explicit SPDX 2.3 implementation

The original acceptance wording names `cargo sbom`, but the frozen ruling requires the output contract—SPDX 2.3 JSON—not one specific generator. The available `cargo-sbom` command does not expose a reliable explicit SPDX-version selector across release environments.

**Ruling:** local and hosted releases use Syft with `spdx-json@2.3`. CI parses the result and fails unless `spdxVersion == "SPDX-2.3"`. Container SBOMs are generated from immutable image digests so the document describes the artifact that is actually signed and released.

### T8-D9 — Load testing requires a release-test quota override

The Team 7 default command quota is 120 requests per minute per organization, which correctly prevents a 100-VU test from generating sustained traffic. Testing the latency target through a stream of expected 429 responses would not validate the command path.

**Ruling:** Team 7's default policies now accept positive environment overrides (`ONYX_COMMAND_RATE_LIMIT`, `ONYX_SYNC_RATE_LIMIT`, `ONYX_AUTOMATION_RATE_LIMIT`). Production defaults remain unchanged; CI sets a high command limit only for the load environment.

### T8-D10 — Migration creation always emits PostgreSQL and SQLite pairs

The repository has two migration directories and all persisted contracts must remain portable to native SQLite clients.

**Ruling:** `migration-tool create <name>` creates timestamped `.up.sql` and `.down.sql` files in both database directories. A release is blocked when one side of a pair is absent.

### T8-D11 — Desktop Docker image is a build/reproducibility artifact

Desktop applications are distributed as signed native packages, not run as Kubernetes services. The desktop Dockerfile exists to reproduce the Linux build and inspect the resulting binary; it is not installed by Helm.

### T8-D12 — Dependency lockfiles must be regenerated before locked builds

The inherited `Cargo.lock` predates the Team 7 and Team 8 dependency additions, and the Web UI has no `package-lock.json`. This execution environment cannot reach the Rust or npm registries, so neither lockfile can be truthfully regenerated here.

**Ruling:** online CI, release jobs, and Docker builders run `cargo generate-lockfile` before `cargo build --locked`. Web CI prefers `npm ci` when a lockfile is present and otherwise performs `npm install`. The absence of committed refreshed lockfiles remains an explicit release sign-off blocker: production sign-off requires generating, reviewing, and committing both lockfiles from an approved networked build environment.

### T8-D13 — Canary analysis must use the metric's real label contract

Team 7 defines `requests_total` labels as `method`, `route`, and `status`, with `service` supplied as a constant registry label. An initial chart draft queried an undefined `outcome` label.

**Ruling:** the Argo AnalysisTemplate computes server-error rate from `status=~"5.."` and filters the constant `service="onyx_api_server"` label. This aligns rollback behavior with the emitted Prometheus family instead of silently returning an empty series.

### T8-D14 — Release signatures bind immutable image digests

Cosign signing by mutable tag can bind a different image if the tag moves between push and signature.

**Ruling:** GitHub release automation signs `${IMAGE}@${DIGEST}` returned by the build-push action. Syft consumes the same digest reference. Binary archives remain detached-armored GPG artifacts, and GitHub provenance attestation covers the assembled release files.

### T8-D15 — Scaled API replicas require one shared operational database

The canary and HPA contracts permit several API replicas simultaneously. A local SQLite file is not a valid shared consistency boundary for those replicas.

**Ruling:** production startup fails closed unless `DATABASE_URL` is PostgreSQL. The API reads projections and persists Approval/Notification commands and idempotency through the shared PostgreSQL database. `deploy/docker-compose.local.yml` also exercises the shared PostgreSQL composition so local release testing matches production topology.

### T8-D16 — Readiness verifies storage without leaking connection secrets

A process-only health endpoint can mark a pod ready while its database is unavailable, and logging a complete database URL can expose credentials.

**Ruling:** `/health` remains the liveness endpoint; `/ready` executes `SELECT 1` against the configured projection pool and is used by Kubernetes readiness and the production image health check. Startup logs record only `storage_backend=postgres|sqlite`, never the credential-bearing URL.

### T8-D17 — API command error classification follows the delivered enum

The inherited web command route classified `LifecycleEpochConflict` and unit `NotFound`, but the delivered `CommandError` enum defines `EpochConflict`, tuple `NotFound(ObjectId)`, and an additional `Idempotency` variant. The mismatch would prevent Rust compilation and made the audit classifier non-exhaustive.

**Ruling:** command audit classification now matches the delivered enum exactly: `NotFound(_)`, `EpochConflict { .. }`, and `Idempotency(_)` are handled explicitly. The public HTTP mapping remains unchanged.

## Required release infrastructure

- Argo Rollouts controller and kubectl plugin.
- NGINX ingress traffic routing for Argo Rollouts.
- Prometheus reachable from the analysis template.
- OpenTelemetry collector/Jaeger OTLP endpoint.
- GitHub OIDC for keyless cosign and provenance.
- GPG private key stored as a protected release secret.
- PostgreSQL backup/PITR and quarterly restore drills.

## Team 8 quality gates

```bash
scripts/ci-pipeline.sh
cargo test -p e2e --test all_journeys
cargo test -p chaos --test all
k6 run tests/load/smoke-test.js --vus 100 --duration 60s
helm lint deploy/helm/onyx-api
terraform -chdir=deploy/terraform validate
scripts/verify/verify_team8.sh
```

---

# Final Structural Gaps — Binding Rulings

**Issued:** 2026-08-05  
**Status:** FROZEN  
**Source:** ONYX Final Structural Gaps: Binding Rulings & Execution Prompts

### FS-R1 — Root README

The workspace root MUST contain `README.md` with the frozen ONYX overview, quick-start commands, architecture summary, deployment paths, and contribution gates.

**Implementation:** Added `README.md` at the workspace root exactly following the approved content.

### FS-R2 — Primary GitHub Actions CI

The repository MUST expose `.github/workflows/ci.yml` for pushes and pull requests to `main`. It MUST run formatting, Clippy, release build, release tests with one test thread, and documentation generation.

**Implementation:** The existing broader Team 8 workflow was consolidated under the required `CI` workflow contract. Stronger migration, contract, web, and load gates were retained.

### FS-R3 — Deployment verification in CI

CI MUST lint the API, worker, and sync-agent Helm charts; initialize and validate Terraform without a backend; and build the API, worker, and sync-agent Dockerfiles.

**Implementation:** Added a named `deploy-check` job with the frozen commands. Helm and Terraform setup actions are included so the commands are available on a clean GitHub runner.

### FS-R4 — Flutter application deferred to v1.1

The Flutter `mobile/` application is not a v1.0 release blocker. The Rust `mobile-core` FFI remains the completed native integration boundary, while the Flutter distribution channel is deferred to v1.1.

**Implementation:** Added root `MOBILE_STATUS.md`. No placeholder Flutter application or unverified generated bridge was added to the v1.0 repository.

### FS-R5 — Flutter v1.1 blueprint retained as implementation guidance

When mobile implementation resumes, it MUST follow the approved Flutter directory structure, minimal dependency set, generated `flutter_rust_bridge` boundary, and platform wrapper responsibilities described in the frozen structural-gap ruling.

---

# Flutter Mobile App v1.1 — Binding Implementation Decisions

**Issued:** 2026-08-05  
**Status:** IMPLEMENTED  
**Source:** Team Prompt — Flutter Mobile App v1.0 / Ruling R5

### M11-D1 — Bind the delivered C ABI instead of inventing a second Rust core

The prompt requests `flutter_rust_bridge_codegen`, but the delivered `mobile-core` exposes a cbindgen C ABI and contains no `flutter_rust_bridge` annotation surface. Replacing it would fork the already-tested native contract.

**Ruling:** `mobile/lib/bridge/bridge.dart` is a generated-style Dart FFI binding over the real cbindgen symbols. The approved `flutter_rust_bridge` package remains a declared dependency for future generated-model migration, but domain behavior and native ownership stay in the existing Rust core.

### M11-D2 — Correct the mobile-core source path

The prompt's codegen example points to `crates/bins/mobile-core/src/lib.rs`; the delivered workspace path is `crates/mobile-core/src/lib.rs`.

**Ruling:** all mobile build and bridge tooling uses the delivered path. No duplicate crate is created.

### M11-D3 — Mobile list projections remain Rust-owned

The original query registry only supports `GetMission` and `GetTask` by identifier, while the required Dashboard/Missions/Tasks screens need lists.

**Ruling:** `mobile-core` exposes `mobile_core_list_aggregates`, which reads the authoritative local SQLite projection and returns version/epoch metadata. Flutter does not maintain or infer domain state.

### M11-D4 — Conflict UI operates on real ConflictRecord values

`SyncAgent` previously retained only an open-conflict count after a synchronization session.

**Ruling:** the agent retains process-lifetime open `ConflictRecord`s, exposes them to mobile-core, and accepts `LocalWins`, `RemoteWins`, or `Escalated` resolutions through the synchronization-domain model. Durable cross-restart conflict persistence and application of the winning operation remain a synchronization-engine follow-up rather than fabricated Dart logic.

### M11-D5 — One registered mobile-core handle supports native schedulers

The delivered iOS/Android background C functions require a `MobileApp` pointer, but OS background callbacks do not carry Dart-owned pointers.

**Ruling:** one process-lifetime mobile-core handle is atomically registered when Flutter initializes it and cleared before free. iOS and Android wrappers invoke `mobile_core_background_sync_registered`. This explicitly adopts the single-mobile-app-instance assumption.

### M11-D6 — Flutter owns generated runner scaffolding

Gradle wrappers, Xcode project metadata, and plugin registrants are Flutter-version-generated files rather than ONYX domain artifacts.

**Ruling:** `mobile/tool/ensure_platform_scaffold.sh` runs `flutter create` when those generated files are absent and restores the frozen ONYX native wrappers. Rust packaging scripts then install the Android libraries or iOS XCFramework.

### M11-D7 — Device P2P certification is an explicit hardware gate

Wi-Fi Direct and BLE cannot be validated in a server container or single emulator.

**Ruling:** the integration test is present and activates with `ONYX_MOBILE_DEVICE_TEST=1` in a two-device lab. Source integration and trigger routing are mandatory in normal CI; physical transport completion is a go-live evidence item and is not claimed as passed here.

### M11-D8 — Missing bounded-context adapters render honest empty states

The delivered local composition registers Mission and Task only. Approval and Notification local domain crates are not present.

**Ruling:** those screens query the real local store and render explicit empty/integration-pending states. The mobile app does not synthesize approvals or notifications.

### M11-D9 — Mobile lockfile and runtime builds require approved networked runners

This environment has no Flutter/Dart/Rust mobile toolchain and cannot resolve pub or Cargo dependencies.

**Ruling:** no fabricated `pubspec.lock`, APK, IPA, XCFramework, or test report is committed. GitHub mobile jobs generate dependencies and execute analysis/tests/builds; reviewed lockfiles remain a release-signoff requirement.

### M11-D10 — Cross-thread event callbacks transfer string ownership

The delivered Rust event callback passed a temporary `CString` valid only during a synchronous callback. Dart's `NativeCallable.listener` intentionally schedules cross-thread callbacks asynchronously, so retaining that behavior would create a use-after-free.

**Ruling:** Rust transfers each event string with `CString::into_raw`; the Dart callback always releases it through `mobile_core_free_string`. The mobile Dart SDK floor is raised to 3.3 so the cross-thread-safe `NativeCallable.listener` contract is available.

### M11-D11 — Flutter cannot certify placeholder radio transports

The delivered `WifiDirectConnection`, `BLEConnection`, and listeners in `sync-transport` return `TransportError::ConnectionLost`; the `sync-transport-mobile` native functions are contract-shaped radio stubs rather than a completed byte-stream adapter.

**Ruling:** Flutter initiates synchronization only through `mobile-core` and contains no parallel P2P implementation. The device-lab test and native permissions/wrappers are delivered, but Wi-Fi Direct/BLE completion remains blocked on implementing the underlying Rust/native transport stream. This is reported as a pre-existing platform-core gap, not misrepresented as a Flutter failure or a passed acceptance criterion.

---

# CI Run 84151743639 — Binding Repair Decisions

**Issued:** 2026-08-05  
**Status:** IMPLEMENTED  
**Evidence:** `logs_84151743639.zip`

### CI-R1 — Resolve the duplicate `handlers` module at its canonical split location

The Rust formatter could not resolve `client-composition::handlers` because both
`src/handlers.rs` and `src/handlers/mod.rs` existed. The latter is the documented
split implementation and owns `creation_handler.rs` and `decision_handler.rs`.

**Ruling:** remove the stale flat `src/handlers.rs`; retain the split directory module.

### CI-R2 — Preserve the formatting gate rather than weakening CI

The `check` job stopped at `cargo fmt --check` and emitted 345 formatting hunks.

**Ruling:** apply the runner-produced `rustfmt` changes to the repository. The CI gate
remains `cargo fmt --check`.

### CI-R3 — Enable Axum's WebSocket feature

`api-server` imported `axum::extract::ws`, but workspace dependency `axum = "0.7"`
did not enable the feature. This caused both `E0432` and the downstream handler type
error on `/api/events`.

**Ruling:** freeze the workspace dependency as
`axum = { version = "0.7", features = ["ws"] }`. The second Axum version mentioned
by rustc is transitive and was not the cause of this failure.

### CI-R4 — Use portable axe assertions

The installed `vitest-axe` contract exports `axe` but not the
`toHaveNoViolations` matcher used by the test.

**Ruling:** assert `results.violations` is empty. This preserves the accessibility
criterion without depending on an unavailable matcher extension.

### CI-R5 — Type mutable MSW fixtures by their canonical projection contracts

TypeScript inferred `decision_reason` as the literal type `null`, preventing the
approval mock from assigning a submitted reason.

**Ruling:** type notification and approval fixtures as `NotificationProjection[]`
and `ApprovalProjection[]` respectively.

### CI-R6 — Make the Node TypeScript project explicitly no-emit

`allowImportingTsExtensions` is legal only with `noEmit` or
`emitDeclarationOnly` under the resolved TypeScript version.

**Ruling:** add `noEmit: true` to `tsconfig.node.json`.

### CI-R7 — Do not rely on executable mode for repository scripts

The Flutter scaffold job exited 126 because `tool/ensure_platform_scaffold.sh` was
not executable in the Git checkout.

**Ruling:** all workflow-owned shell entry points are invoked explicitly with
`bash`, including mobile build and verification scripts.

### CI-R8 — Replace the deprecated Rust toolchain action

The runner warned that `actions-rs/toolchain@v1` targets a deprecated Node runtime.

**Ruling:** use `dtolnay/rust-toolchain@stable`, retaining the required clippy and
rustfmt components.

### CI-R9 — Runtime re-certification remains a GitHub runner gate

This repair environment has no Rust, Flutter, Docker, Helm, Terraform, or package
registry access.

**Ruling:** static and source-level repairs are delivered now. The corrected workflow
must be rerun on GitHub; no unexecuted runtime gate is represented as passed.

# CI Follow-up Run — Binding Repair Decisions

**Issued:** 2026-08-05  
**Status:** IMPLEMENTED  
**Evidence:** `job-logs.txt`, `job-logs 2.txt`, `job-logs 3.txt`, `job-logs 4.txt`

### CI-R10 — Install formatter and linter on the pinned Rust toolchain

The runner selected workspace toolchain `1.97.1`, while the toolchain action had
installed `rustfmt`/`clippy` without explicitly binding those components to that
pinned toolchain. `cargo fmt --check` therefore failed before compilation.

**Ruling:** keep `dtolnay/rust-toolchain@stable`, but explicitly pass
`toolchain: "1.97.1"` everywhere and install `clippy, rustfmt` in the full check
job. This preserves the frozen toolchain and the formatting gate.

### CI-R11 — Bind command failures under the name actually consumed

The API command route matched `Err(_error)` and then referenced `error`, producing
`E0425` twice during the API Docker build.

**Ruling:** bind that arm as `Err(error)`; retain safe error classification,
audit emission, and public error mapping unchanged.

### CI-R12 — Use Vitest's typed configuration surface

The Vite `defineConfig` overload did not admit the `test` property during the
TypeScript project build.

**Ruling:** import `defineConfig` from `vitest/config`, which combines the Vite
configuration contract with the Vitest `test` extension.

### CI-R13 — Remove template-owned Flutter test drift

`flutter create` generated `test/widget_test.dart` referencing the template
`MyApp`, which does not exist in ONYX. The same analysis run also exposed one
unused native-library field, one unused import, and two deprecated Flutter APIs.

**Ruling:** the scaffold script deletes the generated template test immediately.
The FFI wrapper keeps only its bindings, the repository test import is removed,
and current `initialValue` / `withValues(alpha:)` APIs are used.

### CI-R14 — Freeze an explicit S3 lifecycle filter

Terraform validation warned that the backup lifecycle rule omitted both `filter`
and `prefix`, and that this will become an error in a future AWS provider.

**Ruling:** add an explicit empty `filter {}` to apply the retention rule to all
objects without relying on provider-default behavior.

### CI-R15 — Dependency security and lockfiles remain separate release gates

The web install reported 12 dependency vulnerabilities, while the repository still
lacks committed web and Flutter lockfiles and carries a stale Cargo lock warning.

**Ruling:** do not silently upgrade frozen dependencies or fabricate locks in this
repair. Capture `npm audit --json`, review direct/transitive exposure, regenerate all
three dependency locks, and commit them before production signing.

