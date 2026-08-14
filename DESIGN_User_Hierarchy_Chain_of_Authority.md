# ONYX — User Hierarchy & Chain-of-Authority (Design Only, Not Implemented)

**Status:** Design/documentation only, per explicit instruction — no code
changes in this document. Captures requirements given 2026-08-12 for
future implementation.

**Precision standard (confirmed 2026-08-12):** the person was asked
whether the existence of the Allfather override tier (§2.4) means
ambiguous permission edges can be filled with reasonable defaults
rather than fully clarified up front. **Answer: no** — "I still want
reasonable precision on each tier before building, Allfather is just a
safety net." Allfather's correctability is not a license to under-specify;
this document should continue to be built toward real precision on each
tier, not approximations deferred to later correction.

---

## 1. Scope of this requirement

Two related but distinct features:

1. **User administration, Admin-only.** Creating users, deleting users,
   and changing a user's class/type is restricted to the Admin role
   (framed as "line manager or HR chief"). No other role may perform
   these actions, regardless of where that role sits in the hierarchy
   below.
2. **Todo lists & target lists with a verification workflow.** Created
   by staff-level users; **verified** by the creator's parent Manager in
   the chain of authority — an actual approval step, not merely a
   visibility/read permission.

Both are governed by a **hierarchical chain-of-authority** class system,
which the person asked to have documented now and built later.

Terminology: "users" in this system may be labeled employees, staff,
troops, or members depending on organization type — the underlying
mechanism is the same regardless of label; the label itself is likely a
per-organization display setting, not a different code path (to be
confirmed when this is built — see §5).

---

## 2. The class hierarchy (as given, verbatim intent preserved)

| Class | Position | Add/remove staff? | Observation scope | Notes |
|---|---|---|---|---|
| **Admin** | Top of chain (line manager / HR chief) | Yes — sole authority for user creation/deletion/class changes | Everything | Already exists as `is_admin` in the current codebase; maps directly to this position. |
| **Top-level Manager** | One level below Admin | **Yes** — can add or remove | Everything beneath their own chain | |
| **Senior Manager** | Below Top-level Manager | **No** — work-related authority only | Supergroups and subgroups **of their own** | Cannot add/remove staff — narrower than Top-level Manager despite similar naming. |
| **Team Leader** | "Supergroup leader" | No | Own supergroup, workflow-scoped | See §2.2 — clarified in detail; a real, standing authority, not decorative. |
| **Supervisor** | Subgroup-level | No | Own subgroup (observe/monitor only) | Not team-responsible (that's Team Leader). No verification or escalation authority — pure observation. See §2.3. |
| **Staff** | Base level | No | Own scope only | Creates todo/target lists; these require verification by their parent Manager (see §4, and the loan exception in §2.1). |

**Remaining open point:** Senior Manager's exact relationship to
Team Leader (which one sits above the other operationally, if either)
has not been asked yet.

## 2.1 Staff borrowing / loan mechanism (clarified 2026-08-12)

The base hierarchy **is a strict tree** — every staff member has exactly
one true **owning Manager** (the "real owner," their actual line
manager). Confirmed explicitly by the person after considering the
alternative (multiple simultaneous parents) and rejecting it in favor of
a more precise mechanism:

**A time-bounded borrowing/loan system**, layered on top of the tree,
not a change to the tree itself:

- A **different** Manager (the "borrowing manager") can temporarily gain
  working authority over a staff member who is owned by another Manager,
  for a **designated period** (start + end, or start + duration — exact
  representation not yet specified).
- Ownership itself does **not** transfer. The real owner remains the
  real owner throughout; the loan is an overlay, not a reassignment.
- **Consent requirement (confirmed):** initiating a loan requires the
  **real owner's approval** — a higher-authority manager cannot
  unilaterally order a staff member borrowed away from their owner. This
  is itself a small approval workflow (borrow request → owner
  approves/declines), conceptually adjacent to the Todo/Target
  verification workflow in §4, though not confirmed to share the same
  mechanism.
- **Verification during an active loan (confirmed):** either the
  borrowing manager or the real owner may verify the staff member's
  Todo/Target lists while the loan is active — not exclusive to one
  side. This is a deliberate widening of §4's "parent Manager verifies"
  rule for the loan period specifically.
- Presumed (not yet explicitly confirmed) to revert automatically to
  the real owner's sole authority when the loan period ends — needs
  confirmation, not assumed.

**Implications for the data model**, once this is built:
- A `StaffLoan`-shaped aggregate/entity is needed: staff member, real
  owner, borrowing manager, time window, and an approval state
  (Requested → Approved/Declined by owner → Active → Expired/Ended).
- The "who verifies a given list" check becomes: real owner, **or** any
  manager currently holding an active, approved loan for that staff
  member — not a single fixed lookup.
- Time-bounded: needs either a scheduled/background expiry check, or an
  on-read check ("is `now()` within the loan's window") — not yet
  decided which.

## 2.2 Team Leader's concrete authority (clarified 2026-08-12)

Confirmed explicitly: **Team Leader is functional, not a decorative
title** — "he definitely helps the workflow."

**Baseline authority (standing, not delegated):**
- A Team Leader can **pre-check** — review, validate, and check
  documents/evidence/items on Todo/Target lists within their own
  supergroup. This is inherent to the role itself; it does not require
  a Manager to grant it (explicitly ruled out as a delegation
  mechanism — see below).
- This pre-check is **assistance toward the Manager's decision**, not a
  substitute for it: it helps move items "faster towards the
  compilation," i.e. speeds up what reaches the Manager in a
  ready-to-verify state.
- **Confirmed optional/parallel, not a required gate:** a Manager can
  verify a list directly without a Team Leader pre-check ever having
  happened. The pre-check is a real, standing capability, but the
  workflow does not depend on it.

**Final verification authority stays with the real Manager.** A Team
Leader's pre-check is explicitly *not* equivalent to the §4 "parent
Manager verifies" sign-off — two distinct stages exist:
1. Team Leader pre-check (optional, standing baseline authority, no
   delegation needed).
2. Manager final verification (the authoritative sign-off; unaffected
   by whether step 1 happened).

**Explicitly a separate mechanism from delegation and from the §2.1
staff-loan system** — the person was asked directly whether Team Leader
authority should reuse the time-bounded loan/delegation shape, and
declined: Team Leader's pre-check ability is a standing property of the
role, not something borrowed or time-limited from above.

**Not yet resolved:** whether "pre-check" produces its own recorded
state (e.g. a `PreChecked` flag/timestamp visible to the Manager) or is
purely informal/observational with no distinct system state — needs
confirmation before this is modeled as a real workflow step versus just
a permission to view/comment.

## 2.3 Supervisor's concrete authority (clarified 2026-08-12)

**Confirmed: Supervisors are not responsible for teams — Team Leader
is.** This was the key distinguishing clarification: Supervisor is
explicitly a lighter tier than Team Leader, not an overlapping or
parallel one.

**Concrete scope, confirmed:**
- **Observes/monitors their own subgroup** — visibility into their
  subgroup's activity.
- **No verification authority** — cannot verify Todo/Target lists
  (unlike Team Leader's pre-check, §2.2, and unlike a Manager's actual
  verification, §4).
- **No escalation authority** — cannot invoke escalation (§4.1)
  themselves.

This settles the earlier hedge ("probably a complimentary title only")
into something concrete: Supervisor is real but narrow — pure
observation, no action rights over Todo/Target items, no team
ownership. It is not merely a title with zero system meaning (it does
grant subgroup visibility), but it grants materially less than Team
Leader.

## 2.4 "Allfather" — global platform-owner class, above the org hierarchy (added 2026-08-12)

**A new class, sitting above everything else in this document**,
introduced when the person considered how ONYX should generalize its
class/label system across different customer organizations (small
businesses, midsize companies scaling up, military-style orgs wanting
"Troops" instead of "Staff," etc.).

**Fundamentally different in kind from every other class above.**
Admin through Staff (§2) are all **organizational** roles — they exist
*within* one company's use of ONYX, scoped to that organization. Allfather
is **not** an organizational role:

- **Confirmed global, not per-organization:** there is exactly **one**
  Allfather across the entire platform, above every organization — not
  one per company. Explicitly identified as the person themself, in the
  role of platform/product owner, not a customer-facing position any
  organization would have or assign on its own.
- **Confirmed to have override power over the base hierarchy itself,**
  not just additive configuration on top of it: Allfather can edit,
  add, or remove **any class's core abilities** — including stripping
  Admin's own defining power (user creation/removal/class changes,
  §2's "sole authority" framing) if desired. The §2 hierarchy is
  therefore the **default/floor design**, not an immutable contract —
  Allfather can reshape it platform-wide.
- Can customize per-organization display labels (the "Staff vs. Troops
  vs. Members" question that prompted this class's creation in the
  first place).
- Can, per the person's own framing, "customize everything, edit
  everything, add or remove every feature from every class."
- **Confirmed 2026-08-12: entirely outside the operational
  workflow.** Explicitly stated: "Allfather is not something inside the
  workflow... it is not involved inside the projects and workflow."
  Allfather never touches Missions, Tasks, Todos, Targets,
  verifications, escalations, or any other in-project activity — its
  authority is strictly over the **structure/configuration layer**
  (classes, permissions, fields, labels — "the rule, the game theory and
  plan," in the person's words), never over the operational content
  running through that structure. This is an important boundary: it
  means Allfather should not appear anywhere in the Todo/Target,
  verification, or escalation design (§4) as a participant — it is a
  meta-layer above all of it, not a top rung of the same ladder.

**Implications, not yet resolved:**
- Since Allfather can edit *any* organization's class permissions, this
  is effectively a platform-administration capability, likely living
  outside any single organization's data/tenancy boundary entirely —
  needs to be modeled as such (e.g. not just another row in an
  org-scoped `users` table with a very high permission level, but a
  genuinely distinct, cross-tenant capability), though this hasn't been
  confirmed as an implementation approach yet.
- If Allfather can strip Admin's user-management power for a given
  organization, that organization would then have **no one** able to
  manage its own users unless Allfather also assigns that power
  elsewhere — a real edge case to think through before building this
  (does removing a class's core ability require designating a
  replacement, or can an org genuinely end up with a gap?).
- Whether there is ever more than one Allfather (e.g. a small ops team
  on the product-owner side, not just one person) is unconfirmed —
  current wording ("my title actually") suggests exactly one for now.
- Security/audit implications of a single global super-role are
  significant (a compromised Allfather account could reshape every
  customer's permission model) — not yet discussed, worth raising
  before implementation.

---

## 3. Relationship to the existing `is_admin`/`is_manager` implementation

**Current state (implemented, see `DECISIONS.md` "Manager role
(`is_manager`) — Complete"):**
- `is_admin: bool` — already matches this design's "Admin" position
  exactly (sole authority over user management). No change needed here.
- `is_manager: bool` — a flat, single-tier placeholder built before this
  hierarchy was specified. It gates Policy/Settings administration only
  and cannot express the multiple distinct classes above (different
  add/remove rights, different observation scopes, verification
  authority).

**Recommendation given to the person, accepted implicitly by moving to
"document only" (not yet formally re-confirmed as a build instruction):**
`is_manager` should be **superseded** by a proper class/role field (e.g.
`UserClass` enum or a `role`/`class` string with a defined value set)
once this hierarchy is built — not kept running alongside it as a
second, competing concept. `is_admin` stays as-is.

This is flagged here explicitly because `is_manager` is already live in:
- Postgres + SQLite migrations (`20260106000000_add_manager_role`)
- `security_application::ports::user_store::{UserRecord, NewUser,
  UserStore::set_manager}`
- Both DB adapters (`security-adapter`)
- `api-server`'s `UserDto`, `CreateUserRequest`, `SetManagerRequest`,
  `require_manager_or_admin` guard, and the
  `POST /api/admin/users/:id/manager` route

Replacing it is a real migration (new column/table, data backfill
strategy for any existing `is_manager = true` users, updated guards
throughout the chain above) — not a trivial rename. This should be
scoped as its own implementation task when the person is ready to build
it, not folded silently into an unrelated change.

---

## 4. Todo lists & target lists — verification workflow

Requirements as given:
- Created by Staff-level users (and presumably any class, authored
  by whoever owns the item — not yet confirmed as Staff-only).
- **Verified** by the creator's **parent Manager** in the chain — i.e.
  whoever is immediately above the creator in the hierarchy, not any
  Manager, and not necessarily Admin.
- Implies:
  - Each user needs a defined **parent** in the hierarchy (a reporting
    line), not just a flat class label — the class alone doesn't tell
    the system *whose* Manager should verify a given Staff member's
    list.
  - A verification/approval state machine on Todo/Target list items
    (e.g. Draft → Submitted → Verified / Rejected), likely similar in
    shape to the existing `Approval`-adjacent patterns already in the
    codebase (Notification/Approval commands in `api-server`), but this
    has not been confirmed against the blueprint's own Approval bounded
    context (§4.x) — worth checking for overlap before designing a new
    mechanism from scratch.

**Not yet designed:** the actual aggregate/domain model for
Todo/Target lists. No `todo-domain` or `target-domain` crate exists yet.
This is new domain scope beyond the 5 domains already built (Mission,
Work/Task, Communication, File, Policy) — worth checking whether "Todo
list" is meant to be a lighter-weight variant of the existing
`work-domain`'s `Task`, or a genuinely separate concept, before
building anything.

## 4.0.1 Two directions of todo creation (clarified 2026-08-12)

Originally read as purely Staff-authored ("created by Staff-level
users"); clarified to be **bidirectional — both are valid, depending on
the situation:**

1. **Manager-assigned:** a Manager (or presumably any class above
   Staff, per the general hierarchy) creates/assigns a todo item
   directly in the system for a staff member to act on.
2. **Staff-authored, including capturing verbal instruction:** a Staff
   member writes a todo item themselves — including as the formal,
   written record of something that originated **verbally** (a meeting
   decision, a spoken instruction, a troubleshooting conversation). In
   this case, the act of the staff member writing it down is what
   registers/documents the instruction into the system — described by
   the person in management-psychology terms, as reinforcing that the
   staff member has registered and is committing effort toward the
   item.

**Either origin still goes through the same §4 verification step** by
the parent Manager (or an escalated authority, per §4.1) — the
bidirectional creation does not change who verifies, only who may
author the initial item.

**Resolved 2026-08-12: no circularity concern — the same Manager who
assigns a list also verifies it later,** and this is explicitly treated
as legitimate, not a conflict: "that list is a legitimate assignment and
must be verified and approved at the end." A Manager-assigned list is
not exempt from verification.

## 4.0.1.1 Verification granularity — list-level, not item-level (clarified 2026-08-12)

**Verification applies to the whole list as a single unit, not to
individual items one by one.** Confirmed explicitly: "list approvals
are not one by one their items."

**Two outcomes for a list-level verification:**
1. **Flawless approval** — the list is accepted as-is.
2. **Approved with deficiencies** — the list is still approved overall,
   but the Manager attaches **one free-text comment** to the approval
   action, where they elaborate on the issues in detail.

**Confirmed explicitly (asked directly to remove ambiguity):** there is
**no per-item flagging mechanism** — a Manager cannot mark individual
items within the list as deficient. "Deficiencies" exist only as prose
in the one comment attached to the single list-level approval action.

**Implication for the data model:** a Todo/Target list's verification
state machine needs exactly one comment field on the
approve/verify transition (optional or always-present — not yet
specified), not a per-item annotation structure. This is simpler than
an initial reading of "approved with deficiencies" might suggest — no
item-level state is needed for this purpose.

## 4.0.2 Target lists — clarified scope (2026-08-12)

Distinct from Todo lists: **time-bound, measurable goals** (e.g.
"achieve X during this week/month"), where the point is whether the
target was **reached**, not simply whether a task was completed.

**Confirmed explicitly out of scope for the system itself:** any
consequence of hitting a target (recognition as "top staff of the
month," bonuses, etc.) is **real-world/organizational**, not something
ONYX needs to compute, flag, or display. The person was asked directly
whether this should be a trackable feature (leaderboard, badge, report)
and said no. **ONYX's responsibility stops at tracking progress toward
a defined target and whether it was met** — what an organization does
with that outcome is outside this system's concern.

## 4.1 Escalation (clarified 2026-08-12 — a general platform principle, not Todo/Target-specific)

While discussing Todo/Target verification specifically, the person
stated this as a **broad rule spanning almost every authority in the
system**, not a narrow feature of one workflow: *"almost every
authorities must be capable of this escalation."* Recorded here at the
point it was raised, but should be treated as a cross-cutting design
principle to apply wherever an authority/approval check exists
(Todo/Target verification, staff-loan approval in §2.1, and likely
others not yet identified).

**Confirmed shape:**
- **Escalation is always intentional — deliberately invoked by a
  person, never automatic.** Confirmed 2026-08-12, correcting an
  earlier misreading in this document: "both ends" (from "both end of
  the managers could trigger that") meant *either side of an
  escalation* — i.e. either the escalating manager or (implicitly) the
  system on behalf of the receiving authority — can be involved in
  surfacing it, **not** "both automatic and manual trigger types exist."
  There is no automatic/timeout-based escalation in this system at all.
  The earlier "Not yet resolved: what triggers automatic escalation"
  question is now moot — there is no automatic trigger to specify.
- **No explanation/justification required to escalate.** Quoted
  directly: managers "must knew the situation" without needing a
  written cause — escalating is not gated behind justifying why.
- **Step-by-step, not a direct jump.** Escalation always goes to the
  **immediate next level up** in the chain (e.g. parent Manager →
  Senior Manager → Top-level Manager → Admin → Allfather), one level at
  a time — confirmed explicitly, ruling out "jump straight to any
  higher authority."
- **Full visibility on escalation, both directions (confirmed):** when
  something escalates, both the original party and the higher authority
  it escalates to must see full context/history automatically — this
  is *how* "no explanation required" is achieved: the system carries
  the context itself rather than requiring the escalating manager to
  write one. Implies escalated items need their full history/thread
  visible to the receiving authority, not just a bare notification.

**Not yet resolved:**
- Whether escalation is available at *every* single authority/approval
  point in the system, or "almost every" implies some specific,
  not-yet-identified exceptions.
- Whether escalating past a level requires that level's authority to be
  literally unavailable/unresponsive, or can be invoked even when the
  immediate parent is available but the escalating party simply wants a
  higher decision.

## 4.2 Technical recommendation: Todo/Target vs. the existing Approval mechanism (2026-08-12)

The person asked for Claude's engineering judgment on open question 6
rather than stating a product preference. Recorded here as a
**recommendation**, not a confirmed decision — needs the person's
sign-off before implementation.

**What exists today:** `ApprovalAggregate` in
`crates/bins/api-server/src/routes/command.rs` — a single-step, binary
`pending → approved/rejected` object with `requested_by`, a free-text
`target_id`/`target_type` pointer to some other thing, and a reason. It
is **not** a real domain crate (unlike Mission/Task/Communication/
File/Policy, which each have a dedicated `*-domain` crate with full
`AggregateRoot` semantics and test coverage) — it lives inline in
`api-server`'s routing layer as a thin, generic utility.

**Recommendation: build Todo/Target as their own domain crate
(`todo-domain` or similar), not by reusing/extending `ApprovalAggregate`
directly.** Reasoning:

1. **Escalation (§4.1) needs multi-step, chain-aware routing.**
   `ApprovalAggregate` has exactly one decision point and no concept of
   "who's next in the chain" — bending it to support escalation would
   mean rewriting most of it anyway.
2. **Todo/Target carry more state than a binary approval:** actual
   content, an optional Team Leader pre-check stage before verification
   (§2.2), list-level (not item-level) verification with an optional
   deficiency comment (§4.0.1.1), and — for Targets specifically —
   ongoing progress tracking against a measurable goal over a time
   window (§4.0.2), not a single yes/no gate.
3. **Architectural consistency:** every other real business concept in
   this codebase is a first-class domain crate. Routing new business
   logic through what is essentially a routing-layer convenience type
   would be inconsistent with how the rest of the system is built.

**What to actually reuse:** the *pattern*, not the code — a
`pending → approved/rejected`-shaped transition is a reasonable
building block for the list-level verification step specifically, but
a new `todo-domain` crate should define its own richer state machine
(e.g. `Draft → Submitted → [optional TeamLeaderChecked] →
Verified/VerifiedWithDeficiencies/Rejected`, plus `Escalated`
transitions per §4.1), independent of `api-server`'s
`ApprovalAggregate`.

---

## 5. Open questions to resolve before implementation

1. Does every user have exactly one parent Manager (a tree), or can the
   hierarchy be more general (e.g. dotted-line reporting, multiple
   parents)? **Confirmed 2026-08-12: strict tree**, one true owning
   Manager per user — see §2.1 for the full answer, which also
   introduced the time-bounded staff-loan mechanism as the actual
   solution for "shared staff" scenarios, rather than a multi-parent
   tree.
2. Is class assignment itself part of "user creation" (Admin sets class
   at creation time), or a separate action (Admin can promote/demote
   later)? **Confirmed 2026-08-12: both.** Admin may set a class at
   creation time, and may also change a user's class later as a
   separate action. Implies the data model needs class as a mutable
   field on the user record (not creation-only/immutable), and — per
   the earlier "changes user's class and types" wording being Admin-only
   — every such later change is itself an Admin-only, presumably
   audited action (an event/history worth recording, not just an
   in-place overwrite, though an explicit audit-log requirement hasn't
   been confirmed).
3. Team Leader and Supervisor's concrete permissions, as flagged in §2.
   **Both confirmed 2026-08-12** — Team Leader: §2.2. Supervisor: §2.3
   (observe/monitor own subgroup only; no team ownership, no
   verification, no escalation).
4. Should "employees/staff/troops/members" be a per-organization display
   label only, or do different organization types need different
   underlying class *sets* (e.g. a military-style org might want more
   granularity than a small business)? **Effectively resolved
   2026-08-12 via §2.4:** rather than a separate labels-config
   mechanism, the new global **Allfather** class can edit/add/remove
   any class's abilities and (implicitly) labels platform-wide, for any
   organization. Whether Allfather is the *only* path to changing
   labels, or a lighter per-organization label override also exists
   underneath it for routine cases, is not yet specified — worth
   clarifying before implementation, since routing every small labeling
   preference through a single global role seems like a scaling
   concern for the platform owner rather than a per-customer setting.
5. Does Todo/Target list verification block on a single parent Manager
   approval, or can it escalate (e.g. Senior Manager can also verify if
   the direct parent is unavailable)? **Confirmed 2026-08-12 as a
   general platform principle, not just for Todo/Target — see new §4.1
   ("Escalation").**
6. Relationship to the existing Approval-adjacent capability already in
   `api-server` (`notification.Acknowledge`, `approval.Approve/Reject`)
   — is Todo/Target verification meant to reuse that mechanism, or is
   it a distinct workflow? **Answered as a technical recommendation
   2026-08-12, not a product decision — see new §4.2 ("Technical
   recommendation: Todo/Target vs. the existing Approval mechanism").**
   The person asked for Claude's engineering judgment rather than
   stating a preference; recorded as a recommendation pending the
   person's confirmation before implementation.
7. Whether Manager-assigned todo items (§4.0.1) still require
   verification by that same Manager, or skip straight to a different
   completion signal. **Resolved 2026-08-12 — see §4.0.1:** yes, the
   same Manager both assigns and verifies; this is treated as
   legitimate, not circular. This also surfaced the list-level (not
   item-level) verification granularity — see §4.0.1.1.

---

## 6. Explicit non-action taken

Per the person's instruction, **no code was written or modified for
this feature.** No new migration, no new domain crate, no UI changes.
This document exists solely so the requirement is recorded accurately
ahead of a future implementation session, per the earlier standing
instruction to document and register all such decisions in a markdown
file.
