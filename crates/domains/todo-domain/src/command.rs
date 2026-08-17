//! Commands accepted by the `TodoList`, `TargetList`, and `StaffLoan`
//! aggregates.
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §4.0.1,
//! §4.0.1.1, §4.0.2, §2.1.

use crate::state_machine::VerificationOutcome;
use crate::value::{ListOrigin, LoanWindow, TimeWindow, TodoItem};
use platform_kernel::UserId;
use serde::{Deserialize, Serialize};

/// Commands accepted by the `TodoList` aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TodoListCommand {
    /// Create a new todo list. Routed through `TodoList::create()`, not
    /// `decide()` — mirrors every other domain's established
    /// creation-command pattern.
    CreateTodoList {
        /// Who this list is for (the Staff member it will be verified
        /// against). May differ from the creating actor when
        /// `origin` is `ManagerAssigned` (design doc §4.0.1).
        owner: UserId,
        /// Whether a Manager assigned this list or the Staff member
        /// authored it themselves (design doc §4.0.1).
        origin: ListOrigin,
        /// The list's initial items.
        items: Vec<TodoItem>,
    },
    /// Add an item to a `Draft` list. Rejected once the list has been
    /// submitted — a submitted list's content is what gets verified;
    /// changing it after submission would invalidate an in-flight
    /// verification.
    AddItem {
        /// The item to add.
        item: TodoItem,
    },
    /// Submit the list for verification. Transitions `Draft` →
    /// `Submitted`.
    SubmitTodoList,
    /// Record an optional, non-gating Team Leader pre-check. Design doc
    /// §2.2: never blocks or substitutes for real verification below;
    /// Staff must never see this pre-check's substance, only that it
    /// occurred (enforced at the read-model layer, not by this
    /// command/event — see
    /// `IMPLEMENTATION_PLAN_User_Hierarchy.md` D.5).
    RecordTeamLeaderPreCheck {
        /// The Team Leader's private notes. Never exposed to Staff —
        /// see this command's doc comment.
        notes: String,
    },
    /// Verify the list. List-level only — design doc §4.0.1.1 confirmed
    /// explicitly there is no per-item flagging mechanism. The comment
    /// is always optional and independent of `outcome` (resolved
    /// 2026-08-16 — see design doc §4.0.1.1).
    VerifyTodoList {
        /// Flawless or with-deficiencies.
        outcome: VerificationOutcome,
        /// An optional free-text comment, toggled by a checkbox in the
        /// UI (Phase F) — present regardless of `outcome`, never
        /// required by either outcome.
        comment: Option<String>,
    },
    /// Reject the list outright (distinct from "verified with
    /// deficiencies," which is still an approval).
    RejectTodoList {
        /// Why.
        reason: String,
    },
    /// Escalate the list to a higher authority, per design doc §4.1
    /// (Phase E). Modeled here so `TodoList`'s own state machine can
    /// reflect "currently escalated"; the actual escalation
    /// routing/target-selection logic lives in Phase E, not this crate.
    EscalateTodoList {
        /// Why escalation was invoked.
        reason: String,
        /// Who the escalation routes to. Resolved by the application
        /// layer before this command is issued — `todo-domain` has no
        /// access to the org tree needed to compute "one level up from
        /// the normal verifier" (design doc §4.1, resolved 2026-08-16:
        /// escalation routes to the verifying Manager's own parent, one
        /// further hop up the same tree D.4's `verifier_resolution`
        /// already walks). See
        /// `api_server::escalation_resolution::resolve_escalation_target`.
        escalated_to: UserId,
    },
}

/// Commands accepted by the `TargetList` aggregate. Deliberately
/// parallel to `TodoListCommand` — design doc §4.0.2 (resolved
/// 2026-08-16) confirmed a Target's hit/miss judgment is structurally
/// identical to a Todo's flawless/with-deficiencies verification, just
/// over a time window instead of discrete items, so this command set
/// mirrors `TodoListCommand`'s shape with `items`/`AddItem` replaced by
/// a `time_window` and no per-item content.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TargetListCommand {
    /// Create a new target list. Routed through `TargetList::create()`,
    /// not `decide()`.
    CreateTargetList {
        /// Who this target is for.
        owner: UserId,
        /// Manager-assigned or staff-authored (design doc §4.0.1's
        /// bidirectional-creation rule applies identically to targets).
        origin: ListOrigin,
        /// A short description of what "hitting" this target means.
        /// Free text — design doc §4.0.2 confirmed ONYX does not
        /// compute or validate the metric itself, only tracks whether
        /// it was met, so this is descriptive content, not a typed
        /// metric.
        description: String,
        /// The fixed period this target is measured against.
        time_window: TimeWindow,
    },
    /// Submit the target for verification. Transitions `Draft` →
    /// `Submitted`. May be submitted before `time_window.end_at` is
    /// reached; `VerifyTargetList` itself is what enforces the window
    /// has actually closed (see `error::TargetError::WindowNotClosed`).
    SubmitTargetList,
    /// Record an optional, non-gating Team Leader pre-check. Same
    /// semantics as `TodoListCommand::RecordTeamLeaderPreCheck`.
    RecordTeamLeaderPreCheck {
        /// Never exposed to Staff.
        notes: String,
    },
    /// Verify (judge) the target. Design doc §4.0.2, resolved
    /// 2026-08-16: judged at verification time by the parent Manager —
    /// "Number two, I believe, is more reasonable" — not tracked live
    /// while the window is open. `outcome: Flawless` means the target
    /// was hit; `WithDeficiencies` means it was missed. Only valid at
    /// or after `time_window.end_at`.
    VerifyTargetList {
        /// Hit (`Flawless`) or missed (`WithDeficiencies`).
        outcome: VerificationOutcome,
        /// Always optional, independent of outcome — same rule as
        /// `TodoListCommand::VerifyTodoList`.
        comment: Option<String>,
    },
    /// Reject the target list outright (e.g. it was misconfigured and
    /// should never have been submitted) — distinct from a "miss"
    /// outcome, which is still a completed verification.
    RejectTargetList {
        /// Why.
        reason: String,
    },
    /// Escalate to a higher authority, per design doc §4.1.
    EscalateTargetList {
        /// Why.
        reason: String,
        /// See `TodoListCommand::EscalateTodoList`'s `escalated_to` doc
        /// comment — identical rationale.
        escalated_to: UserId,
    },
}

/// Commands accepted by the `StaffLoan` aggregate.
///
/// Source: design doc §2.1, resolved 2026-08-16. **Three distinct
/// approval gates**, each on a different command — do not collapse
/// these into one generic "approve" command, since the approving actor
/// differs per transition:
/// - `RequestStaffLoan` → approved by the **real owner**.
/// - `ExtendStaffLoan` → approved by the **staff member being loaned**.
/// - `EndStaffLoanEarly` → requires **no approval from anyone**.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StaffLoanCommand {
    /// Request a new loan. Routed through `StaffLoan::create()`, not
    /// `decide()`. Starts in `Requested`, awaiting the real owner's
    /// approval (design doc §2.1).
    RequestStaffLoan {
        /// The staff member being loaned.
        staff_user_id: UserId,
        /// The staff member's real (owning) manager, denormalized at
        /// creation time so a later org-tree change doesn't
        /// retroactively alter this historical record (see
        /// `IMPLEMENTATION_PLAN_User_Hierarchy.md` C.1).
        real_owner_id: UserId,
        /// The manager requesting to borrow the staff member.
        borrowing_manager_id: UserId,
        /// Fixed start/end, set at request time (design doc §2.1,
        /// resolved 2026-08-16: "they must be fixed at duration time").
        window: LoanWindow,
    },
    /// The real owner approves a `Requested` loan. Transitions
    /// `Requested` → `Active`. Only the real owner may invoke this —
    /// enforced by the composing application's authority check
    /// (`decide()` here only checks the generic authority stub, per
    /// this crate's established precedent — see `aggregate.rs`'s
    /// header comment).
    ApproveStaffLoan,
    /// The real owner declines a `Requested` loan. Transitions
    /// `Requested` → `Declined`. Terminal.
    DeclineStaffLoan {
        /// Why.
        reason: String,
    },
    /// Extend an `Active` (or already `Extended`) loan past its current
    /// `end_at`. Design doc §2.1, resolved 2026-08-16: requires the
    /// **staff member's own approval**, not either manager's — this
    /// command itself represents that approval having been given (the
    /// composing application is responsible for confirming the actor
    /// invoking this is the staff member in question, same authority
    /// precedent as `ApproveStaffLoan` above).
    ExtendStaffLoan {
        /// The new, later end date-time.
        new_end_at: platform_kernel::Timestamp,
    },
    /// End an `Active` (or `Extended`) loan before its scheduled
    /// `end_at`. Design doc §2.1, resolved 2026-08-16: requires no
    /// approval from anyone — either the real owner or the borrowing
    /// manager may invoke this unilaterally ("Canceling alone is a
    /// normal thing... Everybody going back to their work is a normal
    /// thing").
    EndStaffLoanEarly,
    /// Mark the loan as `Expired` because its `end_at` has been
    /// reached without extension or early end. Invoked only by the
    /// scheduled background job described in design doc §2.1 (resolved
    /// 2026-08-16) — not a user-facing command, but modeled explicitly
    /// so the job's action is a real, auditable domain command rather
    /// than a side-channel mutation.
    ExpireStaffLoan,
    /// Escalates a `Requested` loan's approval decision — design doc
    /// E.2 (resolved 2026-08-16): "any decision made by a Senior
    /// Manager or Top-level Manager" is escalatable, and staff-loan
    /// approval is explicitly listed as an example. Stays `Requested`
    /// (not a separate terminal status) — `escalated_to` widens who
    /// may `ApproveStaffLoan`/`DeclineStaffLoan` to include the real
    /// owner's own parent, mirroring
    /// `TodoListCommand::EscalateTodoList`'s "replaces, doesn't
    /// duplicate" semantics exactly. Confirmed by the person
    /// 2026-08-16: the escalation target gains authority to
    /// approve/decline **in the real owner's place**, not merely a
    /// notification with no effect on who may act.
    EscalateStaffLoan {
        /// Why.
        reason: String,
        /// Who the escalation routes to — one level up the tree from
        /// the real owner (or, on re-escalation, one level up from the
        /// current `escalated_to`). Resolved by the application layer,
        /// same reason as `TodoListCommand::EscalateTodoList`'s
        /// `escalated_to` field — see that field's doc comment.
        escalated_to: UserId,
    },
}
