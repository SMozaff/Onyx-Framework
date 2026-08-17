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

*(Note added 2026-08-15: §2.4's original "Allfather override tier" was
later superseded by a much narrower design — see §2.4 below. The
precision standard quoted above still holds regardless of that change;
it's recorded here as a standing principle for this document, not
something tied specifically to Allfather's now-superseded shape.)*

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

## 2.1 Staff borrowing / loan mechanism (clarified 2026-08-12; mechanics fully resolved 2026-08-16)

The base hierarchy **is a strict tree** — every staff member has exactly
one true **owning Manager** (the "real owner," their actual line
manager). Confirmed explicitly by the person after considering the
alternative (multiple simultaneous parents) and rejecting it in favor of
a more precise mechanism:

**A time-bounded borrowing/loan system**, layered on top of the tree,
not a change to the tree itself:

- A **different** Manager (the "borrowing manager") can temporarily gain
  working authority over a staff member who is owned by another Manager,
  for a **designated period**.
- **Duration representation (confirmed 2026-08-16): fixed start and end
  date/time, decided at creation time.** Not a rolling "start + N days"
  computed length — the Manager sets concrete start and end dates when
  the loan is created.
- Ownership itself does **not** transfer. The real owner remains the
  real owner throughout; the loan is an overlay, not a reassignment.
- **Consent requirement — starting a loan (confirmed):** initiating a
  loan requires the **real owner's approval** — a higher-authority
  manager cannot unilaterally order a staff member borrowed away from
  their owner. This is itself a small approval workflow (borrow request
  → owner approves/declines), conceptually adjacent to the Todo/Target
  verification workflow in §4, though not confirmed to share the same
  mechanism.
- **Verification during an active loan (confirmed):** either the
  borrowing manager or the real owner may verify the staff member's
  Todo/Target lists while the loan is active — not exclusive to one
  side. This is a deliberate widening of §4's "parent Manager verifies"
  rule for the loan period specifically.
- **Extension requires the Staff member's own approval (confirmed
  2026-08-16).** This is a distinct approval gate from loan creation:
  starting a loan is approved by the real owner; **extending** an
  already-active loan is approved by the Staff member being loaned,
  not by either manager. The Staff member is not a passive subject of
  the loan mechanism once extension is on the table.
- **Ending a loan early requires no approval from anyone (confirmed
  2026-08-16).** Either the real owner or the borrowing manager may end
  an active loan before its scheduled end date, unilaterally. Rationale
  (person's own words): returning a staff member to their normal owner
  is not something that harms anyone, so it carries none of the
  friction that starting or extending a loan does.
- **Notifications (confirmed 2026-08-16):** both the real owner, the
  borrowing manager, **and the staff member** are notified twice per
  loan lifecycle — an advance warning **2–3 days before** the loan's end
  date, and a second notification **when the loan actually ends**. The
  end-of-loan notification carries the option to extend (subject to the
  staff member's approval, above) or let the loan end.
- **Expiry mechanism (confirmed 2026-08-16, as a direct consequence of
  the advance-warning requirement): a scheduled background job**, not an
  on-read/live check. A live check (computing "is `now()` within the
  loan window" only when something happens to ask) cannot produce a
  proactive warning 2–3 days ahead of time, since nothing is "reading"
  the loan at that moment — only a periodic background scan can notice
  "this loan ends in 2 days" and fire a notification with no user
  action having triggered it. This resolves what was previously an open
  question in §2.1 and via Phase C.

**Implications for the data model**, once this is built:
- A `StaffLoan`-shaped aggregate/entity is needed: staff member, real
  owner, borrowing manager, fixed start/end date-time, and an approval
  state (Requested → Approved/Declined by owner → Active →
  Extended{approved by staff} / Ended{by either manager, no approval} /
  Expired{by background job at end date}).
- The "who verifies a given list" check becomes: real owner, **or** any
  manager currently holding an active, approved loan for that staff
  member — not a single fixed lookup.
- A background job (see Phase C in the implementation plan) is required
  to scan for loans approaching their end date (2–3 day lookahead) and
  loans that have just reached their end date, firing notifications and
  transitioning state accordingly. This is a durable scheduled job, not
  request-scoped work — see Blueprint §1.14.2's distinction between
  request-scoped subtasks and durable background jobs.

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

## 2.3 Supervisor's concrete authority (clarified 2026-08-12; visibility confirmed 2026-08-15)

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
  themselves, and (§4.1) cannot have escalation invoked against any
  decision of theirs either, since Supervisor produces no binding
  decisions to escalate.

This settles the earlier hedge ("probably a complimentary title only")
into something concrete: Supervisor is real but narrow — pure
observation, no action rights over Todo/Target items, no team
ownership. It is not merely a title with zero system meaning (it does
grant subgroup visibility), but it grants materially less than Team
Leader.

**Confirmed 2026-08-15 — Supervisor's presence is fully visible, and
that's all it needs to be.** Explicitly described as "a complementary
position for the eldest/most experienced member of the team" —
informal, not a systematic authority layer. Unlike the Team Leader
pre-check (§2.2, §4.0.1.1 below), which needs an existence-only
visibility rule because it carries binding-adjacent judgment content,
Supervisor's observation carries **no recorded decision content at
all** — it is an ambient fact ("Supervisor X is present/monitoring this
team"), not an object with substance to gate. No special visibility
rule is needed: full visibility of *presence* is correct and complete,
because there is nothing beneath that presence to protect or hide.
Implication for `todo-domain`'s data model: Supervisor observation is
not a recorded field on Todo/Target items at all, and needs no
per-field visibility rule the way the Team Leader pre-check does.

## 2.4 Global platform-owner capability — resolved 2026-08-15 as a narrow provisioning action, not a standing "Allfather" account

**This section supersedes the original 2026-08-12 "Allfather" design
below in full.** The original design (preserved further down for
historical record — see the boxed note) proposed a single, standing,
globally-scoped account sitting above the entire organizational
hierarchy, with sweeping override power over every class's core
abilities platform-wide. That design is **no longer the plan.**

**What changed:** the actual purpose was clarified 2026-08-15 — ONYX is
intended to be sold to multiple customer organizations, and the
underlying need is (a) per-customer configuration flexibility, and (b)
a way to bring new customer organizations into existence. Both of these
turned out not to require a standing cross-org account:

- **(a) is already solved.** Per-organization configuration
  (feature toggles, limits, retention — the "Lego" framing) is exactly
  what `policy-domain`'s existing, per-org, versioned Policy mechanism
  does, administered by that organization's own Admin through the
  already-built Admin Platform. No cross-org account is needed for this
  at all — each customer configures their own instance.
- **(b) is solved by a single narrow action, not a role.** Reference-
  checked against a well-regarded multi-tenant SaaS pattern before
  deciding (a NestJS multi-tenant starter, checked via Context7):
  confirmed that pattern has no standing cross-tenant super-admin
  account either — tenant creation there is a plain, one-off
  authenticated action, and per-tenant settings are managed by each
  tenant's own owner/admin, never by a vendor-side account reaching
  into live customer data.

**Confirmed shape of the replacement, 2026-08-15:**
- A **narrow, one-off provisioning action**: creates a new organization
  and seeds its first Admin account, then stops. No ongoing or standing
  access into that organization afterward — the new org's own Admin
  takes over immediately, identically to every other organization.
- Invoked by the platform owner (the person) specifically — not
  currently a grantable role others could hold.
- Because this is a rare, one-off action rather than a persistent
  daily-use account, it does **not** need the heavy security apparatus
  a standing account would have required (mandatory MFA policy,
  session-length limits, an unforgeable audit trail, a kill switch
  independent of its own credential) — those concerns applied
  specifically to a standing account, and that account no longer
  exists in this design.
- **Not yet decided:** how the newly-created org's first Admin
  credential is handed to the customer (a temporary password vs. an
  invite/email flow). Small, does not block building the provisioning
  action itself.

**Status: designed, not yet built.** Small in scope — expected to reuse
the org+Admin creation logic already proven out in the existing
bootstrap path (`routes::admin`), exposed as a single provisioning
route/action rather than a new standing role or auth system.

> **Historical record — original 2026-08-12 "Allfather" design,
> superseded above.** Preserved for context on how the design evolved;
> do not implement as written below.
>
> A new class, sitting above everything else in this document,
> introduced when the person considered how ONYX should generalize its
> class/label system across different customer organizations (small
> businesses, midsize companies scaling up, military-style orgs wanting
> "Troops" instead of "Staff," etc.). Proposed as fundamentally
> different in kind from every other class — not organizational, but
> global, with override power over the base hierarchy itself (able to
> edit, add, or remove any class's core abilities platform-wide,
> including stripping Admin's own defining power), per-organization
> label customization, and explicit confirmation it sits entirely
> outside the operational workflow (never touching Missions, Tasks,
> Todos, Targets, verifications, or escalations). Flagged but
> unresolved at the time: where such an account would live outside
> normal tenant boundaries, what happens if Admin's power is stripped
> with no replacement designated, whether more than one such account
> could exist, and — extensively discussed in a later session — the
> security/audit burden of a single global super-role being
> compromised. That security discussion is what ultimately surfaced the
> mismatch between this design's scope and its actual purpose, leading
> to the 2026-08-15 resolution above.

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

**Comment field — resolved 2026-08-16: always optional, and decoupled
from outcome.** The comment is not gated by whether the verification is
flawless or has deficiencies — it is an independent checkbox-triggered
toggle available on **any** verification outcome. The verifier checks a
box if they want to add a comment, and only then does a text field
apply; nothing about the outcome itself requires a comment, including
"approved with deficiencies" (though it is expected there in practice,
it is not system-enforced). Person's own words: "It's not like a
requirement. More like a checkbox. If needs description or comment, you
can hit the checkbox and add a comment."

**Implication for the data model:** a Todo/Target list's verification
state machine needs one **always-optional** comment field on the
approve/verify transition — `Option<String>`, independent of which
outcome (flawless / with-deficiencies) was chosen — not a per-item
annotation structure, and not a field whose presence is required by any
particular outcome.

## 4.0.2 Target lists — clarified scope (2026-08-12); hit/miss mechanism resolved 2026-08-16

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

**Hit/miss is binary, not incremental (confirmed 2026-08-16):** "either
hitting the target or missing the target... cannot partially hit the
target." No running counts, percentage progress, or partial-credit
thresholds — a Target resolves to exactly one of two outcomes.

**Determination mechanism — resolved 2026-08-16: judged at verification
time, not tracked live.** Two options were posed explicitly (live
tracking with an interim "achieved" flag vs. a judgment call made when
the window closes, parallel to Todo's flawless/deficiencies decision);
the person chose the latter: *"Number two, I believe, is more
reasonable."* Consequences:
- **No interim tracking state is needed while a Target is open** — no
  progress flags, no `TargetOutcomeRecorded` event, nothing written
  during the window itself.
- When the time window closes, the verifier (the parent Manager, same
  authority as Todo verification, widened during an active loan per
  §2.1) looks at the real-world outcome and declares hit or miss at
  that point — structurally the same action as Todo's
  flawless/with-deficiencies verification, just applied to a binary
  hit/miss outcome instead.
- **This means `TargetList` does not need a materially different state
  machine from `TodoList`.** It can reuse the same
  `Submitted → Verified{Hit|Miss} / Rejected / Escalated` shape (see
  §4.0.1.1 and §4.1), with two differences: a `time_window` field
  (start/end) instead of discrete checklist items, and no per-item
  content at all — the "hit or miss" judgment stands in for the
  flawless/deficiencies distinction. The optional comment field
  (resolved above) applies identically.

## 4.1 Escalation (clarified 2026-08-12 — a general platform principle, not Todo/Target-specific; scope fully resolved 2026-08-15)

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
  Senior Manager → Top-level Manager), one level at a time — confirmed
  explicitly, ruling out "jump straight to any higher authority." Chain
  stops at Top-level Manager/Admin — per §2.4 (resolved 2026-08-15),
  the platform-owner capability sits entirely outside the operational
  workflow and is never a rung in this chain.
- **Full visibility on escalation, both directions (confirmed):** when
  something escalates, both the original party and the higher authority
  it escalates to must see full context/history automatically — this
  is *how* "no explanation required" is achieved: the system carries
  the context itself rather than requiring the escalating manager to
  write one. Implies escalated items need their full history/thread
  visible to the receiving authority, not just a bare notification.

**Scope — fully resolved 2026-08-15:**
- **Escalation applies to every decision, without exception** — no
  enumerated allow-list of "escalatable decision types" is needed. The
  mechanism is generic over decision content.
- **But it is gated by *who made the decision*, not by what kind of
  decision it is.** Only decisions made by **real Managers** (Senior
  Manager, Top-level Manager) are escalatable. Team Leader (§2.2) and
  Supervisor (§2.3) decisions are **never** escalatable — not because
  those decisions don't matter operationally, but because neither role
  holds real systemic/organizational authority in the first place
  ("verbally maybe, but not systematically"). There is nothing to
  escalate past, since neither role's output was ever a binding
  decision in the system's eyes. This is the same principle already
  present in §2.2 (Team Leader's pre-check explicitly "optional/
  parallel, not a gate") and §2.3 (Supervisor "no escalation
  authority"), now applied consistently to escalation as a whole rather
  than decided per-role in isolation.
- **This resolves both previously-open questions from §5 item 5 below**
  (whether escalation is available at every point or only some) and
  the "immediate parent unavailable vs. simply wants a higher decision"
  question is answered by "no justification required" above — escalation
  never requires proving unavailability, it can be invoked whenever the
  escalating party judges it necessary.

**Direct consequence for `todo-domain`'s data model (2026-08-15):**
Because Team Leader pre-checks are never escalatable and Staff are not
a party to that layer of decision-making, **Staff must never be shown
the substance of a Team Leader's pre-check** — only its existence
(status/timestamp/who performed it), never its content, notes, or any
quality signal. Surfacing the substance would create a visible-but-
unactionable disagreement for Staff to react to, which contradicts the
"no recourse at that layer" design — the correct fix is to never expose
the substance in the first place, not to expose it and separately block
escalation on top of it. This needs to be a field-level visibility rule
on whatever aggregate holds the pre-check (Staff sees existence-only;
Manager/Team Leader see full content), not just role-gating on an
entire endpoint. Supervisor observation needs no equivalent rule — see
§2.3's 2026-08-15 update — because it carries no recorded decision
content to protect in the first place.

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
   granularity than a small business)? **Revised 2026-08-15:** the
   2026-08-12 answer (a global "Allfather" class editing labels
   platform-wide) is superseded — see §2.4. Per-organization labels are
   now expected to be handled the same way other per-org configuration
   is: through that organization's own Policy, administered by its own
   Admin — not through a global platform-owner role. Not yet fully
   specified as a concrete Policy rule shape; worth a short follow-up
   before `todo-domain`/labels work begins, but no longer blocked on a
   platform-owner design.
5. Does Todo/Target list verification block on a single parent Manager
   approval, or can it escalate (e.g. Senior Manager can also verify if
   the direct parent is unavailable)? **Confirmed 2026-08-12 as a
   general platform principle, not just for Todo/Target — see §4.1
   ("Escalation"). Scope fully resolved 2026-08-15** — see §4.1's
   "Scope — fully resolved 2026-08-15" subsection: every decision is
   escalatable, but only when made by a real Manager (not Team Leader or
   Supervisor). No open questions remain on escalation.
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
8. How does a new customer organization actually come into being —
   is there a standing platform-owner account with reach into every
   org, or something narrower? **Resolved 2026-08-15 — see §2.4
   (fully rewritten).** No standing account: a one-off provisioning
   action creates the org and its first Admin, then grants no further
   ongoing access. Per-org configuration (the original motivating
   question) is handled by each org's own Admin via existing Policy,
   not by any cross-org role.
9. When a Target's time window closes, is hit/miss determined live
   (something is marked "done" as it happens) or judged afterward at
   verification time? **Resolved 2026-08-16 — see §4.0.2.** Judged at
   verification time, same shape as Todo's flawless/deficiencies
   decision; no interim tracking state needed while a Target is open.
10. Is the verification comment (Todo/Target approve step) required,
    optional-only-on-deficiencies, or something else? **Resolved
    2026-08-16 — see §4.0.1.1.** Always optional, checkbox-triggered,
    independent of outcome.
11. Loan duration representation — fixed start+end vs. start+length?
    **Resolved 2026-08-16 — see §2.1.** Fixed start and end date/time,
    set at loan creation.
12. Who can extend or end an active staff loan? **Resolved 2026-08-16
    — see §2.1.** Starting a loan: real owner approves. Extending an
    active loan: the staff member being loaned approves (not either
    manager). Ending a loan early: either manager may do so
    unilaterally, no approval required from anyone. Expiry/advance
    warning is handled by a scheduled background job, confirmed as a
    direct consequence of the 2–3 day advance-notification
    requirement.

---

## 6. Implementation status

As of 2026-08-16, all product-level open questions above are resolved,
and the `todo-domain` crate (`crates/domains/todo-domain/`) is **built
and verified**: `TodoList`, `TargetList`, and `StaffLoan` are complete
`AggregateRoot` implementations with 40/40 passing unit tests covering
every state transition, clean `cargo clippy -- -D warnings`, and clean
`cargo doc --no-deps`. See
`IMPLEMENTATION_PLAN_User_Hierarchy.md` §10 ("Build progress") for the
concrete summary and Phase C/D for the design-to-implementation
mapping. Prior to 2026-08-16, per the person's standing instruction to
never assume or guess on undecided items, no code for this feature had
been written; every mechanic that code now depends on was confirmed
directly with the person first (§2.1, §4.0.1.1, §4.0.2 above).

**Not yet built:** the application-layer integration this domain crate
does not own — verifier-resolution logic combining the org tree with
active loans and escalation state (D.4), Team Leader pre-check
visibility redaction at the read-model layer (D.5, §2.2), escalation
routing (Phase E), the scheduled background job for loan
expiry/advance notification (C.3), and any persistence or
composition-root wiring. None of this blocks the domain crate itself,
which has no dependency on it.
