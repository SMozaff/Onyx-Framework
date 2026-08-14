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

---

## 1. Overall sequencing and why

Six phases, ordered by **dependency**, not by the order features were
discussed:

```
Phase A — Class & hierarchy data model      (foundation; everything depends on this)
Phase B — Allfather / platform-admin layer  (depends on A; defines the ceiling A operates under)
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
the top of the *organizational* hierarchy, distinct in kind from
Allfather, which is Phase B).

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

### A.3 — Migration & backfill for existing `is_manager = true` users

Real data-migration decision needed, not just a schema change:
existing users with `is_manager = true` need a `UserClass` value
assigned, since the boolean doesn't tell us *which* class they should
become. **Options, not yet decided — flag to the person before writing
this migration:**
- Default all existing `is_manager = true` users to `TopLevelManager`
  (closest existing behavior — could gate Policy/Settings, which
  `is_manager` did).
- Leave `UserClass` nullable for pre-existing managers and require an
  Admin to explicitly assign a real class before they regain
  manager-level access (safer, but breaks existing managers' access
  until manually fixed).

This is a judgment call with real operational impact and should be
confirmed, not silently picked.

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

## 3. Phase B — Allfather / platform-admin layer

Per design doc §2.4: **global, singular, outside the operational
workflow entirely.** This is architecturally different from everything
in Phase A (which is all per-organization), so it should not be modeled
as "just a higher `UserClass` value."

### B.1 — Where Allfather lives (needs confirmation before building)

Since Allfather is explicitly cross-tenant and not organization-scoped,
putting it in the `users` table (which is implicitly org-scoped
throughout the codebase — confirmed by checking `organization_id`
threading through `api-server`'s routes and tokens) would be
architecturally wrong. **Recommend a separate, small
platform-administration mechanism** — its own table/credential outside
the per-org `users` table, with its own authentication path — rather
than bolting it onto the existing tenant-scoped user model.

**This needs the person's confirmation before implementation** — it's a
bigger structural decision than anything else in this plan (a second
authentication system, effectively), and the design doc itself flags
unresolved security/audit implications (§2.4) that should be discussed
first.

### B.2 — What Allfather can actually do (minimum viable scope)

Per design doc §2.4: edit/add/remove any class's abilities, customize
per-org labels. **Recommend starting narrow:** a small set of
platform-level settings (label overrides per org, and a way to toggle
which `UserClass` variants an org can use) rather than a fully generic
"edit any permission" system, which risks becoming its own large,
under-specified project. Flag to the person as a scope-narrowing
recommendation, not a unilateral cut.

### B.3 — The "orphaned Admin power" edge case (design doc §2.4)

If Allfather strips an org's Admin of user-management power, that org
needs *someone* who can still manage its users. **Recommend:** Allfather
assigning that power must be atomic with removing it — i.e. the action
is "transfer this ability from X to Y," not "remove from X" as a
standalone operation that can leave a gap. Needs person's confirmation.

---

## 4. Phase C — Staff loan mechanism

Per design doc §2.1. Independent of Phase B; depends only on Phase A's
tree structure existing.

### C.1 — New `staff_loan` table/aggregate

Fields: `staff_user_id`, `real_owner_id` (denormalized from the tree at
loan-creation time, so a later change to the org tree doesn't retroactively
alter historical loan records), `borrowing_manager_id`, `start_at`,
`end_at`, `status` (`Requested → Approved/Declined → Active → Expired`).

### C.2 — Owner-approval workflow

Per design doc §2.1's confirmed requirement: loan creation requires the
real owner's explicit approval — model as `Requested` → owner
approves/declines, mirroring the shape (not the code — see design doc
§4.2's recommendation against reusing `ApprovalAggregate` directly)
of a simple approval gate.

### C.3 — Expiry handling (needs a decision)

Design doc §2.1 flags this as unresolved: does the loan expire via a
background/scheduled check, or an on-read check (`is now() within the
window`)? **Recommend on-read check for Phase C's first version** — no
new background job infrastructure needed, correctness only depends on
comparing timestamps at the moment of an access check. A scheduled
job could be added later purely as a UX nicety (e.g. proactively
notifying when a loan is about to expire), not as a correctness
requirement. Flag as a recommendation, confirm before building.

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

### D.2 — Aggregates

- **`TodoList`** — a list of items, owned by a Staff member (or
  assigned by a Manager — design doc §4.0.1's confirmed bidirectional
  creation), belonging to exactly one verification cycle.
- **`TargetList`** — structurally similar but tracks a measurable goal
  over a time window (design doc §4.0.2) rather than discrete items;
  needs its own value type for "the metric and whether it was met" —
  exact shape not yet specified by the person, needs a follow-up
  question before this sub-piece is built (what does "reaching a
  target" mean structurally — a number? a boolean checklist? open).

### D.3 — State machine (per design doc §2.2, §4.0.1.1)

```
Draft
  → Submitted
    → [optional, non-gating] TeamLeaderPreChecked   (design doc §2.2: optional, parallel, not a gate)
    → Verified                                       (flawless — design doc §4.0.1.1)
    → VerifiedWithDeficiencies { comment: String }    (design doc §4.0.1.1: one free-text comment, no per-item flags)
    → Rejected
    → Escalated                                       (Phase E)
```

**Verification is list-level, not item-level** (design doc §4.0.1.1) —
confirmed explicitly; do not build per-item approval state.

### D.4 — Verifier-resolution logic

Given a `TodoList`, resolve who may verify it: the creator's (or
assignee's) parent Manager per Phase A's tree, widened by Phase C's
active-loan check, further widened by Phase E's escalation state. This
resolution logic is a natural chokepoint — build it as one shared
function/service other phases call into, not duplicated per phase.

### D.5 — Team Leader pre-check (design doc §2.2)

Standing authority, not delegated, not a gate. Model as a distinct,
optional event (`TeamLeaderPreChecked`) any Team Leader scoped to that
supergroup can emit, purely informational to whoever verifies later — a
Manager can verify with or without this event ever occurring. **Open
per design doc §2.2:** whether pre-check needs its own visible
flag/timestamp — recommend building it as a real, visible event (cheap
to add, and clearly useful for the Manager to see), but flag as an
assumption to confirm, not a silent decision.

---

## 6. Phase E — Escalation mechanism

Per design doc §4.1 (corrected 2026-08-12 — see §0 above). Depends on
Phase A (tree, for "next level up") and Phase D (something needs
escalating).

### E.1 — Escalation command (single mechanism — always intentional, never automatic)

A command any authority in the chain can invoke on a stuck
Todo/Target/loan-approval item: moves it to the *immediate next level
up* (confirmed step-by-step, not a jump — design doc §4.1), carrying
full context/history so the receiving authority needs no separate
explanation (confirmed requirement). **This is the entire mechanism —
there is no automatic/timeout-based path to build.** Confirmed
2026-08-12: escalation is always deliberately invoked by a person.

### E.2 — Cross-cutting application (design doc §4.1: "almost every authority")

Design doc §4.1 explicitly frames escalation as general, not
Todo/Target-specific — it should also apply to Phase C's loan-approval
step at minimum (an owner not responding to a loan request should be
escalatable too, per the same principle). **Flag to the person:**
confirm which specific approval points beyond Todo/Target and loan
approval need escalation before assuming it's truly universal — "almost
every" was the person's own hedge, not "every."

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
- **F.5 (with Phase B, pending B.1's resolution):** A separate
  Allfather-only administration surface — likely not part of the normal
  Desktop/Web app at all, given it's explicitly outside the operational
  workflow; possibly a distinct internal tool. Needs discussion before
  scoping.

Web scope for all of the above follows the same rule already
established for Phase 1 (Web is read/verify-capable per whatever was
decided for Mission/Task's read-only precedent — needs a fresh decision
per feature, not assumed to inherit that precedent automatically).

---

## 8. What this plan deliberately does not start yet

- Phase B (Allfather) needs the person's confirmation on B.1 (where it
  lives architecturally) before any code — flagged as the single
  highest-uncertainty item in this whole plan.
- Phase D.2's Target metric shape needs one more clarifying question
  before implementation.
- Phase B.1 (where Allfather architecturally lives) and Phase B.2
  (Allfather's exact editable scope) both need the person's
  confirmation before implementation, per §3 above.

---

## 9. Suggested build order (concrete)

1. Phase A (foundation — nothing else is buildable without it)
2. Phase D (todo-domain — the actual feature value; can start once A's
   `UserClass`/tree exist, does not need B or C)
3. Phase C (staff loans — smaller, self-contained, can run in parallel
   with late Phase D)
4. Phase E (escalation — needs both A and D done)
5. Phase F, incrementally alongside each of the above
6. Phase B (Allfather) last, or in parallel by a separate track — it's
   architecturally independent of A/C/D/E (Allfather sits outside the
   operational hierarchy per design doc §2.4) and mainly blocked on the
   person's decision in B.1, not on other phases' code.
