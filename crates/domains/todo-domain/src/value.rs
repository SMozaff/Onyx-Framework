//! Value types for the Todo/Target/StaffLoan bounded context.
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §4, §4.0.1,
//! §4.0.1.1, §4.0.2, and §2.1 (staff loans). All mechanics referenced
//! here were resolved 2026-08-16 — see that document and `DECISIONS.md`
//! for the full resolution history and the person's own words.

use platform_kernel::ObjectId;
use serde::{Deserialize, Serialize};

/// The identity of a `TodoList` aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TodoListId(pub ObjectId);

impl TodoListId {
    /// Generates a new random todo-list identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// The identity of a `TargetList` aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TargetListId(pub ObjectId);

impl TargetListId {
    /// Generates a new random target-list identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// The identity of a `StaffLoan` aggregate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StaffLoanId(pub ObjectId);

impl StaffLoanId {
    /// Generates a new random staff-loan identifier.
    pub fn new_random() -> Self {
        Self(ObjectId::new_random())
    }
}

/// A single item within a `TodoList`. Deliberately free-text content
/// with no per-item status field — design doc §4.0.1.1 confirmed
/// explicitly that verification applies to the whole list, never to
/// individual items ("list approvals are not one by one their items").
/// An item is a content record only, not a sub-workflow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    /// A short, stable identifier for this item within its list, used
    /// only for display/ordering — not a target of any command, since
    /// there is no per-item verification (§4.0.1.1).
    pub item_id: String,
    /// The item's content, free text. Deliberately opaque, same
    /// rationale as `policy_domain::value::PolicyRule::key` staying
    /// free text — this crate defines the list/verification mechanism,
    /// not a taxonomy of task content.
    pub description: String,
}

/// Who authored a `TodoList` — the bidirectional-creation rule
/// confirmed in design doc §4.0.1: either a Manager assigns a list to a
/// Staff member, or the Staff member authors it themselves (including
/// as the written record of a verbal instruction). Recorded on the
/// aggregate so verification logic and UI can distinguish origin
/// without inferring it from `created_by` alone (a Manager could in
/// principle also author a list for themselves in a future extension,
/// though no such workflow exists yet — this field keeps origin
/// explicit rather than assumed from actor class).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListOrigin {
    /// A Manager (or any class with assignment authority) created this
    /// list and assigned it to the owning Staff member.
    ManagerAssigned,
    /// The owning Staff member authored this list themselves.
    StaffAuthored,
}

/// The fixed time window a `TargetList` is measured against. Design doc
/// §4.0.2, resolved 2026-08-16: targets are time-oriented, and hit/miss
/// is judged once this window closes — see `state_machine::VerificationOutcome`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeWindow {
    /// When the target period begins.
    pub start_at: platform_kernel::Timestamp,
    /// When the target period ends. Verification (hit/miss) may only
    /// occur at or after this instant — enforced by the aggregate, not
    /// left to caller discipline.
    pub end_at: platform_kernel::Timestamp,
}

/// A `StaffLoan`'s fixed time window. Distinct type from `TimeWindow`
/// above (rather than reusing it) because loans and targets are
/// conceptually different things that happen to share a start/end
/// shape; keeping them separate avoids accidentally coupling a future
/// change to one's semantics to the other's.
///
/// Design doc §2.1, resolved 2026-08-16: "they must be fixed at
/// duration time" — a loan's start and end are concrete dates set at
/// creation, not a rolling `start + length` computed value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoanWindow {
    /// When the loan begins.
    pub start_at: platform_kernel::Timestamp,
    /// When the loan is scheduled to end, absent an early end or an
    /// approved extension (§2.1's `StaffLoanCommand::ExtendLoan`
    /// replaces this value with a new, later `end_at`).
    pub end_at: platform_kernel::Timestamp,
}

/// A recorded Team Leader pre-check, stored on the `TodoList`/
/// `TargetList` aggregate itself once `RecordTeamLeaderPreCheck` is
/// applied. Design doc §2.2, visibility rule resolved 2026-08-15:
/// existence (`checked_by`, `checked_at`) must always be visible, but
/// `notes` (the substance) must never be visible to Staff — this is
/// enforced at the read-model/query layer
/// (`api_server::query_handler::redact_team_leader_pre_check_for_viewer`),
/// not by omitting the field here. This struct stores the full record;
/// redaction is a serialization-time concern for a specific viewer, not
/// a property of the stored data itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TeamLeaderPreCheck {
    /// Who performed the pre-check.
    pub checked_by: platform_kernel::UserId,
    /// When.
    pub checked_at: platform_kernel::Timestamp,
    /// The Team Leader's notes. Never shown to Staff — see this
    /// struct's doc comment.
    pub notes: String,
}
