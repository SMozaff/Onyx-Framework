# ONYX — Implementation Plan: User Hierarchy & Chain of Authority

**Status:** Proposed implementation plan, awaiting go-ahead. No code
written yet — this plan is the bridge between
`DESIGN_User_Hierarchy_Chain_of_Authority.md` (what was decided) and
actual engineering work.

**Source of truth for requirements:**
`DESIGN_User_Hierarchy_Chain_of_Authority.md` — every item below cites
the section it implements. Do not re-derive requirements from memory;
if this plan and that document disagree, the design document wins and
this plan needs updating.

---

## 0. Status of previously-open points

Both items previously tracked here are now resolved:
- **Supervisor's permissions** — confirmed (design doc §2.3): observe/
  monitor own subgroup only, no team ownership, no verification, no
  escalation authority.
- **Escalation's trigger** — confirmed (design doc §4.1, corrected
  2026-08-12): escalation is **always intentional, always
  human-invoked**. There is no automatic/timeout-based escalation in
  this system. An earlier draft of this plan (and the design document
  it's based on) briefly tracked "automatic escalation's trigger
  condition" as an open question — that was a misreading of "both ends
  of the managers could trigger that" (which meant either party
  involved in an escalation, not "automatic and manual as two trigger
  types"). Corrected here; Phase E below reflects the corrected,
  simpler, manual-only design.
- **Target hit/miss determination** — confirmed 2026-08-16 (design doc
  §4.0.2): judged at verification time by the parent Manager, not
  tracked live. Binary outcome, no interim tracking state needed.
- **Verification comment field optionality** — confirmed 2026-08-16
  (design doc §4.0.1.1): always optional, checkbox-triggered,
  independent of outcome — applies to both `TodoList` and `TargetList`.
- **Staff loan duration representation** — confirmed 2026-08-16 (design
  doc §2.1): fixed start/end date-time set at creation, not a rolling
  duration.
- **Staff loan extend/end authority** — confirmed 2026-08-16 (design
  doc §2.1): starting requires real owner approval; extending requires
  the staff member's own approval; ending early requires no approval
  from anyone.
- **Staff loan expiry mechanism** — confirmed 2026-08-16 (design doc
  §2.1): scheduled background job, required by the 2–3 day
  advance-notification rule; on-read checking alone cannot satisfy it.

As of 2026-08-16, **no open product questions remain** for Phase C or
Phase D. Implementation may proceed.

---

## 1. Overall sequencing and why

Six phases, ordered by **dependency**, not by the order features were
discussed:

```
Phase A — Class & hierarchy data model      (foundation; everything depends on this)
Phase B — Platform-owner org-provisioning   (depends on A only loosely; small, independent)
Phase C — Staff loan mechanism              (depends on A)
Phase D — todo-domain crate                 (depends on A; independent of B, C)
Phase E — Escalation mechanism              (depends on A, D; touches C's loan-approval flow too)
Phase F — UI (Desktop + Web where relevant) (depends on A–E as each lands)
```

A, then D, are the critical path — nothing else can be built or even
fully designed against a moving target until users have a real class
field and Todo/Target has a real aggregate.

---

## 2. Phase A — Class & hierarchy data model

**Supersedes `is_manager`,** per design doc §3's explicit recommendation
already given. `is_admin` is untouched.

### A.1 — New `UserClass` enum (replaces `is_manager: bool`)

```rust
pub enum UserClass {
    TopLevelManager,
    SeniorManager,
    TeamLeader,
    Supervisor,  // confirmed 2026-08-12: observe/monitor own subgroup only — no team ownership, no verification, no escalation (design doc §2.3)
    Staff,
}
```
`is_admin: bool` stays a separate field — Admin is confirmed as its own
thing, not a `UserClass` variant (design doc §2's table treats Admin as
the top of the *organizational* hierarchy — distinct in kind from the
platform-owner provisioning action in Phase B, which sits outside any
org entirely rather than above Admin within one).

### A.2 — Reporting-line field (parent Manager)

New nullable `parent_user_id` (or `manager_id`) column on `users`,
self-referential. Required for:
- Todo/Target verification routing (design doc §4 — "verified by the
  creator's parent Manager").
- Escalation's "next level up" resolution (design doc §4.1).

**Enforce tree shape at the database level where possible** (a user
cannot be its own ancestor) — this needs a cycle check, most practically
done in application code at assignment time rather than a DB constraint,
since SQL doesn't express "no cycles" declaratively. Flag this as a
real validation to write, not assume it "just works."

### A.3 — Migration & backfill for existing `is_manager = true` users — decided

**Decided (see `DECISIONS.md`):** the second option below — leave
`class` nullable for pre-existing managers, require an Admin to
explicitly assign a real class per user, no auto-promotion. Rationale:
safer (no class asserted that wasn't actually decided by a person), and
no live production user base yet makes the manual step low-cost.
Documented as a runbook: `docs/runbooks/user-class-migration.md`. This
is deliberately *not* automated as a migration default.

Options considered (kept here for context on why the choice was made):
- ~~Default all existing `is_manager = true` users to `TopLevelManager`~~
  (rejected — asserts a class no one actually chose).
- **Leave `UserClass` nullable for pre-existing managers and require an
  Admin to explicitly assign a real class before they regain
  manager-level access** — chosen.

### A.4 — Update every `is_manager` call site

Per `DECISIONS.md`'s own inventory of where `is_manager` currently
lives — all of these need updating in the same change, not
incrementally (a half-migrated permission model is worse than the
current one):
- Postgres + SQLite migrations (add `UserClass`/`parent_user_id`,
  eventually drop `is_manager` once backfilled — two-step migration:
  add new columns, backfill, then drop old column in a later release,
  not all in one migration, to allow rollback room).
- `security_application::ports::user_store` (`UserRecord`, `NewUser`,
  replace `set_manager` with `set_class`).
- Both DB adapters (`security-adapter`).
- `api-server`'s `UserDto`, `CreateUserRequest`, replace
  `SetManagerRequest` with `SetClassRequest`, replace
  `require_manager_or_admin` with a class-aware guard (see A.5).

### A.5 — Permission-check helper redesign

`require_manager_or_admin` was a simple boolean OR. This needs to
become genuinely class-aware — e.g. "does this action require
`TopLevelManager` or above," parameterized by the calling route, not a
single fixed guard. Design this as a small authorization helper
(`fn require_class_at_least(min: UserClass, ...)` or similar) rather
than one-off checks scattered per route.

**Estimated size:** largest single phase in this plan — touches
migrations, three crates, and every existing manager-gated route.

---

## 3. Phase B — Platform-owner org-provisioning action (redefined 2026-08-15, was "Allfather / platform-admin layer")

**Superseded plan below the line.** Per design doc §2.4 (rewritten
2026-08-15), the original standing "Allfather" account concept — global,
singular, with sweeping override power over every class's abilities
platform-wide — is no longer the design. It was solving a problem that
doesn't actually exist once the real purpose (selling ONYX to multiple
customer orgs) was clarified: per-org configuration is already solved by
existing Policy administered by each org's own Admin, and the only
genuinely vendor-side need is bringing a new org into existence.

**New, much smaller scope for this phase:**

### B.1 — A single provisioning action, not a new auth system

No second authentication system, no cross-tenant credential store, no
standing account. Concretely: one action that, given a new
organization's name and a first Admin's identity/credentials, creates
the `organizations` row and the first `users` row (with class = Admin)
atomically, then grants no further access to the invoking party. This
is expected to reuse the existing bootstrap logic already proven out in
`routes::admin` (the same path used to seed the very first user in any
fresh database) rather than building new plumbing.

### B.2 — Who can invoke it

The platform owner (the person), specifically — not a `UserClass`
variant, not a grantable role. Simplest correct implementation: gate
behind the same kind of bootstrap-token mechanism already used
elsewhere (`ONYX_BOOTSTRAP_TOKEN`), or an equivalent narrowly-scoped
secret held only by the person — not a login session, since this isn't
meant to be a standing, day-to-day account.

### B.3 — What's deliberately NOT being built

No per-org label/class-set editing capability, no "edit any org's
permissions" mechanism, no cross-tenant reach into a live org after
provisioning. If per-organization class-set customization (e.g. a
military-style org wanting "Troops" instead of "Staff," design doc §5
item 4) becomes a real need later, the current expectation is that it
routes through that org's own Policy — a follow-up to specify as a
Policy rule shape, not a reason to resurrect the standing-account
design.

### B.4 — Open, non-blocking

How the newly-provisioned org's first Admin credential reaches the
customer (temporary password vs. invite/email flow) is not yet decided.
Does not block building B.1–B.2.

---

## 4. Phase C — Staff loan mechanism (all mechanics resolved 2026-08-16)


Per design doc §2.1. Independent of Phase B; depends only on Phase A's
tree structure existing.

### C.1 — New `staff_loan` table/aggregate

Fields: `staff_user_id`, `real_owner_id` (denormalized from the tree at
loan-creation time, so a later change to the org tree doesn't retroactively
alter historical loan records), `borrowing_manager_id`, `start_at`,
`end_at` (both fixed date-times, set at creation — confirmed 2026-08-16,
not a rolling duration), `status`:

```
Requested
  → Declined                          (real owner declines)
  → Approved → Active                 (real owner approves)
    → Extended { approved_by: staff_user_id }   (staff member approves — see C.2)
    → Ended { ended_by: manager_id }             (either manager, no approval — see C.2)
    → Expired                                    (background job, at end_at — see C.3)
```

### C.2 — Approval workflow (three distinct gates, confirmed 2026-08-16)

Per design doc §2.1, this is **not** a single approval gate — there are
three separate authority checks depending on which action is taken:

1. **Starting a loan:** requires the **real owner's** explicit approval
   — model as `Requested` → owner approves/declines, mirroring the
   shape (not the code — see design doc §4.2's recommendation against
   reusing `ApprovalAggregate` directly) of a simple approval gate.
2. **Extending an active loan:** requires the **staff member's own**
   approval — not either manager's. This is a distinct command/approval
   path from loan creation; do not reuse the owner-approval gate for
   this. The staff member is the approving party here specifically.
3. **Ending a loan early:** requires **no approval from anyone.** Either
   the real owner or the borrowing manager may end an active loan
   unilaterally, before `end_at`. Model as a direct state transition
   with no pending/approval intermediate state — there is nothing to
   approve.

### C.3 — Expiry handling and notifications (resolved 2026-08-16: background job)

Design doc §2.1 previously flagged this as unresolved between a
background job and an on-read check. **Now decided: a scheduled
background job**, confirmed as a direct consequence of the notification
requirement below — an on-read check cannot produce a proactive warning
before anyone has interacted with the loan.

**Notification requirement (confirmed 2026-08-16):** the real owner,
borrowing manager, **and the staff member** are all notified:
- **2–3 days before `end_at`** — an advance warning, giving the option
  to extend (subject to the staff member's approval, per C.2) before
  the loan lapses.
- **At `end_at`** — a second notification when the loan actually ends,
  again with the option to extend or let it lapse.

Implementation: a periodic scheduled job (see design doc §2.1's
reference to Blueprint §1.14.2 — durable background work, not
request-scoped) that scans `staff_loan` rows for:
1. `Active` loans with `end_at` 2–3 days out and no advance-warning
   notification yet sent → fire advance warning, mark as sent.
2. `Active` loans with `end_at` at or past `now()` → transition to
   `Expired`, fire the end-of-loan notification.

### C.4 — Verification-authority check update

The "who can verify this staff member's list" check (built in Phase D)
must be updated to check: real owner, **or** any manager with a
currently-active, approved loan for that staff member — per design doc
§2.1's confirmed "either can verify" rule. This is a Phase D dependency,
noted here since it originates from this phase's data.

---

## 5. Phase D — `todo-domain` crate

Per design doc §4.2's recommendation (new domain crate, not an
extension of the existing `ApprovalAggregate`). This is the biggest new
domain-modeling effort in this plan.

### D.1 — Crate scaffold

Follow the exact established pattern from `policy-domain` (the most
recently built domain crate in this codebase) — `aggregate.rs`,
`command.rs`, `event.rs`, `error.rs`, `state_machine.rs`, `value.rs`,
`test_support.rs`, full inline test coverage for every transition.

### D.2 — Aggregates (Target mechanics resolved 2026-08-16)

- **`TodoList`** — a list of items, owned by a Staff member (or
  assigned by a Manager — design doc §4.0.1's confirmed bidirectional
  creation), belonging to exactly one verification cycle.
- **`TargetList`** — structurally similar but tracks a **binary**
  hit/miss outcome over a fixed time window (design doc §4.0.2) instead
  of discrete items. **Resolved 2026-08-16:** no running metric, no
  partial-credit threshold — "either hitting the target or missing the
  target." No interim tracking state or event is needed while the
  window is open; the hit/miss judgment is made by the verifier when
  the window closes, structurally identical to `TodoList`'s
  flawless/with-deficiencies decision (see D.3). Given this, `TargetList`
  can very likely **share `TodoList`'s state machine and verification
  code path** rather than needing a parallel implementation — the only
  structural differences are a `time_window { start_at, end_at }` field
  in place of discrete checklist items, and the verifier's outcome
  choice being `Hit`/`Miss` instead of `Verified`/`VerifiedWithDeficiencies`.
  Worth confirming during implementation whether this is close enough
  to unify as one aggregate with a `kind: Todo | Target` discriminant,
  or two thin wrapper types sharing one inner state machine — an
  implementation-detail choice, not a product one.

### D.3 — State machine (per design doc §2.2, §4.0.1.1; comment field resolved 2026-08-16)

```
Draft
  → Submitted
    → [optional, non-gating] TeamLeaderPreChecked   (design doc §2.2: optional, parallel, not a gate)
    → Verified { outcome: Flawless | WithDeficiencies, comment: Option<String> }
    → Rejected
    → Escalated                                       (Phase E)
```

**Comment field (resolved 2026-08-16):** the comment is **always
optional** and **independent of outcome** — not gated to only appear on
`WithDeficiencies`. Model as a single `Option<String>` on the
`Verified` transition regardless of which outcome was chosen, toggled
by a checkbox in the UI (Phase F), not a required field on either
outcome. This replaces the earlier `VerifiedWithDeficiencies { comment:
String }` variant (which incorrectly implied the comment was mandatory
for that outcome) — the outcome (flawless/with-deficiencies, or
hit/miss for `TargetList`) and the comment are now two independent
pieces of data on the same transition.

**Verification is list-level, not item-level** (design doc §4.0.1.1) —
confirmed explicitly; do not build per-item approval state.

### D.4 — Verifier-resolution logic

Given a `TodoList`, resolve who may verify it: the creator's (or
assignee's) parent Manager per Phase A's tree, widened by Phase C's
active-loan check, further widened by Phase E's escalation state. This
resolution logic is a natural chokepoint — build it as one shared
function/service other phases call into, not duplicated per phase.

### D.5 — Team Leader pre-check (design doc §2.2; visibility rule resolved 2026-08-15)

Standing authority, not delegated, not a gate. Model as a distinct,
optional event (`TeamLeaderPreChecked`) any Team Leader scoped to that
supergroup can emit, purely informational to whoever verifies later — a
Manager can verify with or without this event ever occurring. Building
it as a real, visible event (timestamp + who performed it) is
confirmed correct, not just an assumption — see the visibility rule
below.

**Visibility rule — confirmed 2026-08-15, must be enforced at the field
level, not just route-level role gating:** Staff may see that a
pre-check occurred (status/timestamp/who), but must **never** see its
substance (any notes, quality judgment, or outcome detail the Team
Leader recorded). This is a direct consequence of two confirmed rules:
(1) Team Leader decisions are never escalatable (§4.1/E.2 below), and
(2) Staff are not a party to that layer of decision-making at all.
Exposing substance would create a visible-but-unactionable disagreement
for Staff with no system-level remedy, which the design deliberately
avoids by never exposing the substance in the first place — this is not
solved by omitting an escalation button, it must be solved by the read
model itself. Concretely: whatever aggregate/projection holds
`TeamLeaderPreChecked`'s content needs two serialization shapes (or a
field-level redaction step) — full content for Manager/Team Leader
readers, existence-only for Staff readers. Get this confirmed as part
of D.2's aggregate/event design before building the read side, since
retrofitting field-level redaction after a flat projection already
exists is more disruptive than designing it in from the start.

**Supervisor requires no equivalent rule.** Per design doc §2.3
(2026-08-15 update): Supervisor's presence is fully visible and carries
no recorded decision content — it is not a field on Todo/Target items
at all, just an ambient fact, so there is nothing to redact.

---

## 6. Phase E — Escalation mechanism

Per design doc §4.1 (corrected 2026-08-12 — see §0 above; scope fully
resolved 2026-08-15). Depends on Phase A (tree, for "next level up")
and Phase D (something needs escalating).

### E.1 — Escalation command (single mechanism — always intentional, never automatic)

A command any authority in the chain can invoke on a stuck
Todo/Target/loan-approval item: moves it to the *immediate next level
up* (confirmed step-by-step, not a jump — design doc §4.1), carrying
full context/history so the receiving authority needs no separate
explanation (confirmed requirement). **This is the entire mechanism —
there is no automatic/timeout-based path to build.** Confirmed
2026-08-12: escalation is always deliberately invoked by a person.
Chain terminates at Top-level Manager — per design doc §2.4 (rewritten
2026-08-15), there is no platform-owner rung above it to escalate into.

### E.2 — Cross-cutting application — scope fully resolved 2026-08-15 (was "design doc §4.1: 'almost every authority'")

**Resolved: escalation applies to every decision, gated by who made the
decision, not by decision type.** No enumerated list of "escalatable
decision types" needed — the command in E.1 is generic over any
Manager-authored decision. Concretely:
- **Escalatable:** any decision made by a Senior Manager or Top-level
  Manager — Todo/Target verification, staff-loan approval (Phase C),
  and any future Manager-level decision this plan doesn't yet enumerate,
  with no separate confirmation needed per new decision type.
- **Never escalatable:** any Team Leader or Supervisor output (the
  pre-check in D.5, Supervisor's observation) — not because these are
  unimportant, but because neither role holds real systemic/
  organizational authority ("verbally maybe, but not systematically" —
  the person's own framing). There is nothing to escalate past.
- **Implementation implication:** the escalation command (E.1) should
  authorize based on the **actor class that authored the decision being
  escalated**, checked against a fixed allow-list (Senior Manager,
  Top-level Manager), not based on the decision's type/aggregate. This
  is a single, simple guard applied uniformly — do not build separate
  per-decision-type escalation eligibility checks.

No remaining open questions on escalation scope — previously flagged
"confirm which specific approval points" and "unavailable vs. simply
wants a higher decision" are both resolved (see design doc §4.1).

---

## 7. Phase F — UI

Builds incrementally as each backend phase lands; not one big-bang UI
effort.

- **F.1 (with Phase A):** Desktop Settings page gains class assignment
  (replacing the old manager-toggle UI), parent-Manager assignment.
- **F.2 (with Phase C):** A loan request/approval UI — likely on the
  Settings or a new People-management page.
- **F.3 (with Phase D):** New Todo/Target pages — list creation (staff
  and manager-assigned flows), the verification action (flawless /
  with-deficiencies-plus-comment), Team Leader pre-check UI.
- **F.4 (with Phase E):** Escalation action button/flow on any
  verifiable item; an "escalated to you" view for the receiving
  authority.
- **F.5 (with Phase B):** Admin Platform gets a small "Provision new
  organization" action (name + first Admin details in, org created).
  Not a separate administration surface — folds into the existing
  Admin Platform (`admin-shell`) we already built, gated behind the
  bootstrap-token-style mechanism from B.2, not a normal login-gated
  page any Admin could reach.

Web scope for all of the above follows the same rule already
established for Phase 1 (Web is read/verify-capable per whatever was
decided for Mission/Task's read-only precedent — needs a fresh decision
per feature, not assumed to inherit that precedent automatically).

---

## 8. What this plan deliberately does not start yet

- Phase B.4 (how a newly-provisioned org's first Admin credential
  reaches the customer) is small and non-blocking but still open.
- The application-layer integration work listed in §10 below (build
  progress) — verifier-resolution logic, Team Leader pre-check
  redaction, escalation routing, the loan-expiry background job, and
  persistence/composition-root wiring — none of which block the
  `todo-domain` crate itself, which is now built.

---

## 9. Suggested build order (concrete)

1. Phase A (foundation — nothing else is buildable without it)
2. Phase D (todo-domain — the actual feature value; can start once A's
   `UserClass`/tree exist, does not need B or C)
3. Phase C (staff loans — smaller, self-contained, can run in parallel
   with late Phase D)
4. Phase E (escalation — needs both A and D done)
5. Phase F, incrementally alongside each of the above
6. Phase B (org provisioning) — small and self-contained, can run
   whenever convenient; no longer blocked on any open design decision
   (resolved 2026-08-15 — see design doc §2.4), so it can move earlier
   in this order if a new customer org is needed sooner than the rest
   of this plan lands.

---

## 10. Build progress

**`todo-domain` crate — built and verified 2026-08-16.** Ahead of
Phase A in the sequencing above, since `TodoList`/`TargetList`/
`StaffLoan` are self-contained pure-domain aggregates with no
dependency on Phase A's `UserClass`/tree data model (that dependency
belongs to the *application layer's* verifier-resolution logic, per
D.4/C.4 — the domain crate itself only needs `UserId`s, not resolved
org-tree relationships). Location: `crates/domains/todo-domain/` —
registered in the workspace root `Cargo.toml`, following the exact
`policy-domain` scaffold pattern (`aggregate.rs`, `command.rs`,
`event.rs`, `error.rs`, `state_machine.rs`, `value.rs`,
`test_support.rs`, `lib.rs`).

Implements all three aggregates from D.2/C.1 as full `AggregateRoot`s:
- `TodoList` — `Draft → Submitted → [TeamLeaderPreChecked] → Verified /
  Rejected / Escalated`, list-level verification only, always-optional
  comment independent of outcome.
- `TargetList` — the same shared state machine
  (`state_machine::ListStatus`) as `TodoList`, with a `TimeWindow`
  instead of items and a `WindowNotClosed` guard preventing
  verification before the window closes.
- `StaffLoan` — `Requested → Active/Declined →
  Extended/Ended/Expired`, with the three distinct approval gates from
  C.2 modeled as separate commands (`ApproveStaffLoan` by the real
  owner, `ExtendStaffLoan` representing the staff member's approval,
  `EndStaffLoanEarly` requiring no approval), plus
  `grants_verification_authority_to()` as the one piece of D.4's
  verifier-resolution logic that belongs on this aggregate itself.

**Verified:** `cargo check`, `cargo clippy -- -D warnings`, and
`cargo doc --no-deps` all clean; 40/40 unit tests pass, covering every
state transition (valid and rejected) for all three aggregates,
including the window-boundary edge case
(`verify_target_at_exact_window_close_succeeds`) and both directions of
the always-optional-comment rule.

**Superseded by §11 below (2026-08-16):** the "not yet built" list that
previously appeared here (verifier-resolution, redaction, escalation,
the background job, persistence wiring) is now out of date — most of it
was built the same day. See §11 for the current, accurate status.

---

## 11. Backend integration — built and verified 2026-08-16

Everything below was built in one continuous session, in dependency
order, following the person's explicit instruction to fix things "in
technical and feasibility order" and, after a scope check partway
through, to continue with "D.4 + D.5 (backend logic only, no UI)."

### 11.1 — Discovery: Phase A was already complete

Before building anything, Phase A (§2 above) was found **already fully
implemented** from an earlier, uncatalogued session — `UserClass`,
`parent_user_id`, cycle detection, both DB adapters, `require_class`.
Confirmed via a fresh `cargo test` run (24/24 passing) rather than
assumed from reading code alone. This changed the plan: Phase A moved
from "to build" to "to verify," and the rest of this section proceeds
from that corrected starting point.

### 11.2 — `api-server` HTTP integration (C.1/C.2/D.1-D.3's reachability)

- `ApiState` gained `todo_list_repo`/`target_list_repo`/`staff_loan_repo`,
  wired identically to `policy_repo`/`legal_hold_repo`'s existing
  pattern (both Postgres and SQLite branches).
- `/api/command` dispatch: every `todo_list.*`/`target_list.*`/
  `staff_loan.*` decide-routed command, using whole-payload
  deserialization into the command enum (`serde_json::from_value`)
  rather than manual field extraction, since these commands carry
  richer typed fields than the string-only payloads most existing
  arms handle.
- `routes::todo_admin` (new module): `POST /api/todo/lists`,
  `/api/todo/targets`, `/api/todo/staff-loans` for the three
  `create()`-routed commands, mirroring `routes::policy_admin` exactly.
  Deliberately **not** admin-gated — design doc §4.0.1 confirms
  bidirectional Todo/Target creation and §2.1 has no Admin-only
  restriction on requesting a loan, so these routes only require
  authentication.
- `/api/query`: `todo_list.list/.detail`, `target_list.list/.detail`,
  `staff_loan.list/.detail` registered in `query_handler`'s
  `aggregate_type` match and `is_detail` list.
- **Bug found and fixed via live testing**: `issue_token`'s JWT
  `scope.command_types` allow-list is a second, independent
  authorization gate from `routes::command`'s dispatch match — missing
  a command type there 403s with `COMMAND_NOT_AUTHORIZED` even when
  routing is otherwise correct. All new command types added.
- **Verified live** (SQLite backend, real running server, real HTTP
  requests): create → submit → verify → re-query showed
  `status: "Verified"`; staff-loan request → approve succeeded.

### 11.3 — C.3, the staff-loan background job

`crates/bins/worker/src/staff_loan_scheduler.rs` (new): mirrors
`scheduler_loop::scheduler_tick_postgres`'s exact shape. Scans
`aggregates` where `aggregate_type = 'staff_loan'` for two conditions:
`end_at` within `ADVANCE_WARNING_LEAD` (2.5 days — the midpoint of the
confirmed "2 or 3 days," since the person gave a range without
preferring an endpoint) and no warning sent yet; and `end_at` reached.
Enqueues `StaffLoanAdvanceWarning`/`StaffLoanExpiry` jobs via the
existing `JobQueue`/`worker_application` infrastructure — no new job
infrastructure needed, this reuses what Increment 7 already built.

Two new handlers in `crates/bins/worker/src/job_runner.rs` execute
those jobs: insert one `notification` aggregate row per recipient
(staff member, real owner, borrowing manager — design doc §2.1's
confirmed three-party notification) with a role-labeled message, and
(expiry only) transition the loan's `status` to `Expired` via direct
SQL plus a `domain_events`/`outbox` insert — the same "mutate
`aggregates.state` directly for scheduled work" pattern
`execute_timeline_trigger` already established, not a new one invented
for this feature.

**Verification status — explicit, not overstated**: compiles clean,
clippy clean. The exact JSON shape this SQL depends on
(`state->>'status'` as a bare string, `state->'window'->>'end_at'` as a
nested bare integer) was confirmed against `todo_domain::StaffLoan`'s
real serialized output via a throwaway integration test, not assumed.
The `jsonb_set`/`create_missing` Postgres semantics were checked
against PostgreSQL's own documentation. **What was not done**: running
this SQL against a live Postgres instance — none was available in the
build sandbox, and disk space was too constrained (as low as 372MB free
at points) to safely install one. This is a real gap, flagged
explicitly rather than implied to be covered by the other testing done
this session — before relying on this job in a Postgres-backed
deployment, run it once against a real database and confirm the two
`UPDATE ... jsonb_set(...)` statements behave as documented.

### 11.4 — D.4, verifier-resolution

`crates/bins/api-server/src/verifier_resolution.rs` (new):
`resolve_verifiers(owner_id, ...)` combines Phase A's tree
(`UserStore.parent_user_id`) with Phase C's active-loan widening
(`StaffLoan::grants_verification_authority_to`, confirmed the one piece
of this logic already living on the aggregate — see that method's own
doc comment). `is_authorized_verifier(candidate, owner_id, ...)` wraps
it for the common single-actor check. **Deliberately excludes Phase E's
escalation widening** — Phase E has no routing/target-selection code
yet, so adding a stub would look handled while silently doing nothing;
the module's docs say this explicitly rather than pretending
completeness.

Wired into `/api/command`'s dispatch via a new
`require_verifier_authority` helper: `VerifyTodoList`/
`RejectTodoList`/`EscalateTodoList` (and the `TargetList` equivalents)
now load the target aggregate, extract `owner`, and reject the command
before dispatch if the caller isn't an authorized verifier.

**Verified live, both directions**: an unrelated third user's verify
attempt was rejected with `"actor is not an authorized verifier for
this list's owner"`; the real tree-parent Manager's identical request
succeeded and the verification was recorded.

### 11.5 — D.5, Team Leader pre-check visibility redaction

**A real pre-existing bug was found and fixed first**: `TodoList`'s and
`TargetList`'s `apply()` methods discarded
`TeamLeaderPreCheckRecorded`'s `notes`/`checked_by`/`checked_at`
entirely via a `{ .. }` match pattern — the pre-check's substance never
reached the aggregate's persisted state, so there was nothing for any
redaction logic to redact. Fixed: added `todo_domain::value::TeamLeaderPreCheck`
and a `team_leader_pre_check: Option<TeamLeaderPreCheck>` field to both
aggregates, with `apply()` now populating it correctly. Two new
regression tests confirm this
(`aggregate::tests::team_leader_pre_check_then_verify_succeeds`'s
strengthened assertions, plus the new
`target_list_team_leader_pre_check_is_stored`).

With that fixed, `query_handler::redact_team_leader_pre_check_for_viewer`
(new) removes `team_leader_pre_check.notes` specifically when the
querying viewer's id equals the list's `owner` — i.e. Staff viewing
their own list — leaving `checked_by`/`checked_at` untouched. This
matches design doc §2.2's "existence visible, substance never" rule by
construction: a list's `owner` is by definition the Staff member the
pre-check is about, regardless of `origin` (Manager-assigned vs.
staff-authored never changes who `owner` is), so no separate
`UserClass` lookup is needed to identify "the Staff member this concerns."
`execute_query` gained an `Option<ObjectId>` `viewer` parameter,
`None`-safe for every other query type this function serves (mission,
task, policy, etc. are entirely unaffected).

**Verified live, both directions**: querying the list as its Staff
owner returned `team_leader_pre_check` with `checked_at`/`checked_by`
present and no `notes` key at all; querying the identical list as the
Team Leader who performed the check returned the full record including
`notes` verbatim.

### 11.6 — What remains, accurately as of 2026-08-16

- **No UI work** — `desktop-shell`, `admin-shell`, `web-ui` have no
  todo/target/loan screens. Explicitly out of scope for this session's
  work, per the person's own choice.
- **Phase E's actual escalation routing/target-selection logic** —
  `EscalateTodoList`/`EscalateTargetList` record that escalation was
  invoked and why; nothing resolves *who* an escalation goes to. D.4's
  verifier-resolution module has a documented gap here rather than a
  silent one.
- **The C.3 background job's SQL, unverified against real Postgres** —
  see §11.3's caveat above. This is the one piece of this session's
  work that was not live-tested, and should be the first thing checked
  before this job runs against a production Postgres deployment.
- **Phase B.4** (how a new org's first Admin credential reaches the
  customer) — unchanged, still small and open, unrelated to this
  session's work.


