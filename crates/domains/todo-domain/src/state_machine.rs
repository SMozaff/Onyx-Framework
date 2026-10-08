//! State machines for the Todo/Target/StaffLoan bounded context.
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §4.0.1,
//! §4.0.1.1, §4.0.2, §2.1, §4.1. All variants and transitions below
//! were confirmed by the person, most recently 2026-08-16 — see
//! `DECISIONS.md` for direct quotes and `IMPLEMENTATION_PLAN_User_Hierarchy.md`
//! Phase C/D for the corresponding build guidance.

use serde::{Deserialize, Serialize};

/// A `TodoList` or `TargetList`'s lifecycle status. Both aggregates
/// share this single state machine — design doc §4.0.2 (resolved
/// 2026-08-16) confirmed a `TargetList`'s hit/miss judgment is
/// structurally identical to a `TodoList`'s flawless/with-deficiencies
/// verification, just applied to a binary time-bound outcome instead of
/// discrete items, so one shared state machine serves both rather than
/// two parallel ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ListStatus {
    /// Being authored; not yet submitted for verification.
    Draft,
    /// Submitted, awaiting verification by the parent Manager (or an
    /// active-loan manager per design doc §2.1, or an escalated
    /// authority per §4.1).
    Submitted,
    /// An optional, non-gating Team Leader pre-check has been recorded.
    /// Design doc §2.2: this never blocks or substitutes for the real
    /// Manager verification below — it exists in parallel. Staff must
    /// never see the substance of this pre-check, only that it
    /// occurred — enforced at the read-model/serialization layer, not
    /// by this state machine (see `IMPLEMENTATION_PLAN_User_Hierarchy.md`
    /// D.5).
    TeamLeaderPreChecked,
    /// Verified by the parent Manager (or equivalent authority). The
    /// outcome (flawless vs. with-deficiencies, or hit vs. miss for a
    /// `TargetList`) and an always-optional comment are recorded
    /// on the `Verified` event, not as separate statuses — see
    /// `VerificationOutcome` below and design doc §4.0.1.1 (resolved
    /// 2026-08-16: the comment is a checkbox-triggered, always-optional
    /// field, independent of which outcome was chosen).
    Verified,
    /// Rejected by the verifying authority. Terminal in this crate's
    /// scope, mirroring `policy_domain::LegalHoldStatus::Released`'s
    /// precedent of not reopening a decided aggregate — a corrected
    /// list is a new submission, not a mutation of the rejected one.
    Rejected,
    /// Escalated to a higher authority per design doc §4.1 (Phase E).
    /// Modeled here as a status so a list's read model can show
    /// "currently escalated," but the actual escalation routing/target
    /// selection lives in Phase E's escalation mechanism, not in this
    /// crate.
    Escalated,
}

/// The outcome recorded when a list is verified. Shared by `TodoList`
/// (flawless/with-deficiencies) and `TargetList` (hit/miss) — design
/// doc §4.0.2 confirmed these are the same structural decision applied
/// to different content, so one enum serves both rather than two
/// separate ones with duplicated comment-handling logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationOutcome {
    /// `TodoList`: the list is accepted as-is, no issues noted.
    /// `TargetList`: the target was reached within its time window.
    Flawless,
    /// `TodoList`: accepted overall, but the verifier has noted
    /// deficiencies (see the always-optional comment on the `Verified`
    /// event).
    /// `TargetList`: the target was missed.
    WithDeficiencies,
}

/// A `StaffLoan`'s lifecycle status.
///
/// Source: design doc §2.1, resolved 2026-08-16. Three distinct
/// approval gates apply to different transitions out of `Active` — see
/// `command::StaffLoanCommand`'s docs for which actor may invoke each
/// one; this enum only records the resulting states.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StaffLoanStatus {
    /// Requested; awaiting the real owner's approval. Design doc §2.1:
    /// starting a loan always requires the real owner's explicit
    /// approval — a higher-authority manager cannot unilaterally
    /// borrow a staff member away from their owner.
    Requested,
    /// The real owner declined the request. Terminal.
    Declined,
    /// The real owner approved and the loan's window has begun. Both
    /// the real owner and the borrowing manager may verify the staff
    /// member's Todo/Target lists while a loan is `Active` (design doc
    /// §2.1's "either can verify" rule).
    Active,
    /// The loan was extended past its original `end_at`. Requires the
    /// staff member's own approval (design doc §2.1, resolved
    /// 2026-08-16: "for extension, the employee itself need to approve
    /// that") — not either manager's. Functionally still an active
    /// loan; modeled as a distinct status so the read model can
    /// distinguish an original-term loan from an extended one without
    /// inspecting event history.
    Extended,
    /// Ended before its scheduled `end_at`, by either the real owner or
    /// the borrowing manager, unilaterally. Design doc §2.1, resolved
    /// 2026-08-16: "Canceling alone is a normal thing... Everybody
    /// going back to their work is a normal thing" — no approval is
    /// required from anyone for this transition. Terminal.
    Ended,
    /// Reached its scheduled `end_at` without being extended or ended
    /// early. Set by the scheduled background job described in design
    /// doc §2.1 (resolved 2026-08-16 as a direct consequence of the
    /// 2–3 day advance-notification requirement — see
    /// `IMPLEMENTATION_PLAN_User_Hierarchy.md` C.3). Terminal.
    Expired,
}
