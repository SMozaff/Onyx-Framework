//! Domain events emitted by the `TodoList`, `TargetList`, and
//! `StaffLoan` aggregates.
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §4.0.1,
//! §4.0.1.1, §4.0.2, §2.1.

use crate::state_machine::VerificationOutcome;
use crate::value::{
    ListOrigin, LoanWindow, StaffLoanId, TargetListId, TimeWindow, TodoItem, TodoListId,
};
use platform_kernel::{Timestamp, UserId};
use serde::{Deserialize, Serialize};

/// Events emitted by the `TodoList` aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TodoListEvent {
    /// A new todo list was created.
    TodoListCreated {
        /// The new list's identity.
        todo_list_id: TodoListId,
        /// Who it's for.
        owner: UserId,
        /// Manager-assigned or staff-authored.
        origin: ListOrigin,
        /// Initial items.
        items: Vec<TodoItem>,
        /// Who created it (may differ from `owner` — design doc §4.0.1).
        created_by: UserId,
        /// When.
        created_at: Timestamp,
    },
    /// An item was added to a `Draft` list.
    TodoItemAdded {
        /// The item added.
        item: TodoItem,
        /// When.
        added_at: Timestamp,
    },
    /// The list was submitted for verification.
    TodoListSubmitted {
        /// When.
        submitted_at: Timestamp,
    },
    /// A Team Leader pre-check was recorded. Design doc §2.2 — never
    /// gating, never visible to Staff in substance.
    TeamLeaderPreCheckRecorded {
        /// The Team Leader's notes. Handling code at the read-model
        /// layer must redact this field for Staff-facing views — see
        /// `IMPLEMENTATION_PLAN_User_Hierarchy.md` D.5. Recorded here
        /// in full because event storage is not itself a Staff-facing
        /// read model; redaction is a projection concern.
        notes: String,
        /// Who performed the pre-check.
        checked_by: UserId,
        /// When.
        checked_at: Timestamp,
    },
    /// The list was verified.
    TodoListVerified {
        /// Flawless or with-deficiencies.
        outcome: VerificationOutcome,
        /// Always optional, independent of `outcome` (resolved
        /// 2026-08-16 — design doc §4.0.1.1).
        comment: Option<String>,
        /// Who verified it.
        verified_by: UserId,
        /// When.
        verified_at: Timestamp,
    },
    /// The list was rejected outright.
    TodoListRejected {
        /// Why.
        reason: String,
        /// Who rejected it.
        rejected_by: UserId,
        /// When.
        rejected_at: Timestamp,
    },
    /// The list was escalated to a higher authority (design doc §4.1).
    TodoListEscalated {
        /// Why.
        reason: String,
        /// Who invoked escalation.
        escalated_by: UserId,
        /// Who the escalation routes to — one level up the tree from
        /// the normal verifier. See
        /// `command::TodoListCommand::EscalateTodoList`'s doc comment.
        escalated_to: UserId,
        /// When.
        escalated_at: Timestamp,
    },
}

/// Events emitted by the `TargetList` aggregate. Deliberately parallel
/// to `TodoListEvent` — see `command::TargetListCommand`'s doc comment
/// for why a shared shape is appropriate here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TargetListEvent {
    /// A new target list was created.
    TargetListCreated {
        /// The new list's identity.
        target_list_id: TargetListId,
        /// Who it's for.
        owner: UserId,
        /// Manager-assigned or staff-authored.
        origin: ListOrigin,
        /// What hitting this target means, free text.
        description: String,
        /// The fixed period this target is measured against.
        time_window: TimeWindow,
        /// Who created it.
        created_by: UserId,
        /// When.
        created_at: Timestamp,
    },
    /// The target was submitted for verification.
    TargetListSubmitted {
        /// When.
        submitted_at: Timestamp,
    },
    /// A Team Leader pre-check was recorded. Same semantics as
    /// `TodoListEvent::TeamLeaderPreCheckRecorded`.
    TeamLeaderPreCheckRecorded {
        /// Never exposed to Staff in substance.
        notes: String,
        /// Who performed the pre-check.
        checked_by: UserId,
        /// When.
        checked_at: Timestamp,
    },
    /// The target was verified (judged hit or miss). Design doc
    /// §4.0.2, resolved 2026-08-16: this judgment happens once, at
    /// verification time — there is no earlier "outcome recorded"
    /// event, since hit/miss is not tracked live.
    TargetListVerified {
        /// `Flawless` = hit; `WithDeficiencies` = missed.
        outcome: VerificationOutcome,
        /// Always optional, independent of `outcome`.
        comment: Option<String>,
        /// Who verified it.
        verified_by: UserId,
        /// When.
        verified_at: Timestamp,
    },
    /// The target list was rejected outright.
    TargetListRejected {
        /// Why.
        reason: String,
        /// Who rejected it.
        rejected_by: UserId,
        /// When.
        rejected_at: Timestamp,
    },
    /// The target list was escalated (design doc §4.1).
    TargetListEscalated {
        /// Why.
        reason: String,
        /// Who invoked escalation.
        escalated_by: UserId,
        /// See `TodoListEvent::TodoListEscalated`'s `escalated_to` doc
        /// comment — identical rationale.
        escalated_to: UserId,
        /// When.
        escalated_at: Timestamp,
    },
}

/// Events emitted by the `StaffLoan` aggregate.
///
/// Source: design doc §2.1, resolved 2026-08-16.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StaffLoanEvent {
    /// A loan was requested.
    StaffLoanRequested {
        /// The new loan's identity.
        staff_loan_id: StaffLoanId,
        /// The staff member being loaned.
        staff_user_id: UserId,
        /// The staff member's real owning manager.
        real_owner_id: UserId,
        /// The manager requesting the loan.
        borrowing_manager_id: UserId,
        /// Fixed start/end at request time.
        window: LoanWindow,
        /// Who requested it (normally the borrowing manager).
        requested_by: UserId,
        /// When.
        requested_at: Timestamp,
    },
    /// The real owner approved the loan.
    StaffLoanApproved {
        /// Who approved it (the real owner).
        approved_by: UserId,
        /// When.
        approved_at: Timestamp,
    },
    /// The real owner declined the loan. Terminal.
    StaffLoanDeclined {
        /// Why.
        reason: String,
        /// Who declined it (the real owner).
        declined_by: UserId,
        /// When.
        declined_at: Timestamp,
    },
    /// The loan was extended past its previous `end_at`. Design doc
    /// §2.1, resolved 2026-08-16: this action represents the staff
    /// member's own approval, not either manager's.
    StaffLoanExtended {
        /// The new, later end date-time.
        new_end_at: Timestamp,
        /// The staff member who approved the extension — recorded
        /// explicitly (rather than assumed) so downstream consumers do
        /// not need to re-derive "who is the staff member" from the
        /// aggregate's other fields.
        approved_by_staff: UserId,
        /// When.
        extended_at: Timestamp,
    },
    /// The loan was ended before its scheduled `end_at`. Design doc
    /// §2.1, resolved 2026-08-16: no approval required from anyone;
    /// either the real owner or the borrowing manager may end it.
    StaffLoanEndedEarly {
        /// Who ended it (the real owner or the borrowing manager).
        ended_by: UserId,
        /// When.
        ended_at: Timestamp,
    },
    /// The loan reached its scheduled `end_at` without extension or
    /// early end. Fired by the scheduled background job (design doc
    /// §2.1, resolved 2026-08-16).
    StaffLoanExpired {
        /// When (should equal the loan's `end_at`, recorded separately
        /// in case the background job's scan cadence introduces a
        /// small delay — see
        /// `IMPLEMENTATION_PLAN_User_Hierarchy.md` C.3).
        expired_at: Timestamp,
    },
    /// The loan's approval decision was escalated. See
    /// `command::StaffLoanCommand::EscalateStaffLoan`'s doc comment.
    StaffLoanEscalated {
        /// Why.
        reason: String,
        /// Who invoked escalation.
        escalated_by: UserId,
        /// Who the escalation routes to — gains authority to
        /// approve/decline in the real owner's place.
        escalated_to: UserId,
        /// When.
        escalated_at: Timestamp,
    },
}
