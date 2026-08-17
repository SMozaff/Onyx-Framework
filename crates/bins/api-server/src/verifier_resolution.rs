//! Verifier-resolution logic: given a `TodoList`/`TargetList`, who is
//! authorized to verify it?
//!
//! Source: `IMPLEMENTATION_PLAN_User_Hierarchy.md` D.4: "Given a
//! `TodoList`, resolve who may verify it: the creator's (or assignee's)
//! parent Manager per Phase A's tree, widened by Phase C's active-loan
//! check, further widened by Phase E's escalation state. This
//! resolution logic is a natural chokepoint — build it as one shared
//! function/service other phases call into, not duplicated per phase."
//!
//! # Scope of this module, as built 2026-08-16
//! - **Phase A tree widening: built.** The owner's `parent_user_id`
//!   (from `security_application::UserStore`) is always an authorized
//!   verifier.
//! - **Phase C active-loan widening: built.** Per design doc §2.1's
//!   confirmed "either can verify" rule — if the list's owner is
//!   currently on an `Active` or `Extended` `StaffLoan`, both the real
//!   owner and the borrowing manager are authorized (not just one).
//!   This uses `todo_domain::StaffLoan::grants_verification_authority_to`
//!   directly — the one piece of this logic that already lived on the
//!   aggregate itself (see that method's own doc comment for why).
//! - **Phase E escalation widening: built.** Confirmed by the person
//!   2026-08-16: once a specific list is `Escalated`, its own
//!   `escalated_to` (set by
//!   `escalation_resolution::resolve_escalation_target` at the moment
//!   escalation was invoked) **replaces** the tree parent as that
//!   list's sole authorized verifier — matching the design doc's own
//!   language, "an escalated authority" replacing "the parent Manager."
//!   This widening is necessarily per-list, not per-owner like the
//!   other two: it requires knowing the *specific list's* escalation
//!   state, so `resolve_verifiers` takes an optional
//!   `escalated_to: Option<ObjectId>` parameter the caller supplies
//!   from the list it already has loaded, rather than this module
//!   re-deriving it from a fresh lookup.
//!
//! This module deliberately depends on both `security_application`
//! (for the tree) and `todo_domain` (for loans) but is not itself part
//! of either crate — exactly the "application-layer, not domain-layer"
//! placement `todo_domain::lib`'s own crate docs call for.

use std::sync::Arc;

use platform_kernel::ObjectId;
use security_application::UserStore;
use todo_domain::aggregate::StaffLoan;

use crate::query_handler::{fetch_rows, ProjectionPool};

/// Who may verify a list owned by `owner`, and why. Multiple entries
/// are possible (tree parent, plus up to two loan-derived verifiers if
/// `owner` is currently on an active loan where neither party happens
/// to already be the tree parent — an edge case, but not one this
/// function should silently under-report).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizedVerifier {
    pub user_id: ObjectId,
    pub reason: VerifierReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifierReason {
    /// The owner's parent Manager per Phase A's reporting-line tree.
    TreeParent,
    /// The real owner named on an active/extended `StaffLoan` covering
    /// this owner. Present even when this user is *also* the tree
    /// parent — see `resolve_verifiers`'s own dedup note.
    LoanRealOwner,
    /// The borrowing manager named on an active/extended `StaffLoan`
    /// covering this owner.
    LoanBorrowingManager,
    /// This specific list's `escalated_to` target (Phase E) — replaces
    /// `TreeParent` for this list only, per design doc §4.1's "an
    /// escalated authority" replacing "the parent Manager."
    EscalationTarget,
}

#[derive(Debug, thiserror::Error)]
pub enum VerifierResolutionError {
    #[error("owner user id is not a valid UUID")]
    InvalidOwnerId,
    #[error("user store lookup failed: {0}")]
    UserStore(String),
    #[error("staff loan projection query failed: {0}")]
    Projection(String),
    #[error("staff loan state is not valid JSON for todo_domain::StaffLoan: {0}")]
    Deserialization(String),
}

/// Resolves the set of users authorized to verify a list owned by
/// `owner_id`, within `organization_id`. See this module's doc comment
/// for exactly which widenings are and are not included.
///
/// `escalated_to`: pass `Some(target)` when the *specific list* being
/// resolved for is currently `Escalated` with that target (read it off
/// the loaded `TodoList`/`TargetList` via its own `escalated_to()`
/// accessor) — this replaces the tree parent for this call, per Phase
/// E's confirmed semantics. Pass `None` for a non-escalated list, which
/// preserves this function's exact prior behavior.
///
/// Returns an empty `Vec` (not an error) if `owner_id` has no tree
/// parent and no active loan — an owner with genuinely no authorized
/// verifier is a real, valid state the caller must handle (e.g. a
/// Top-level Manager authoring their own list has no parent above
/// them), not a failure of this function.
pub async fn resolve_verifiers(
    owner_id: ObjectId,
    organization_id: uuid::Uuid,
    user_store: &Arc<dyn UserStore>,
    staff_loan_repo_pool: &ProjectionPool,
    escalated_to: Option<ObjectId>,
) -> Result<Vec<AuthorizedVerifier>, VerifierResolutionError> {
    let mut verifiers = Vec::new();

    // --- Phase E: escalation widening (replaces the tree parent) ---
    if let Some(escalation_target) = escalated_to {
        verifiers.push(AuthorizedVerifier {
            user_id: escalation_target,
            reason: VerifierReason::EscalationTarget,
        });
    } else {
        // --- Phase A: tree parent (only when NOT escalated) ---
        let owner_uuid = uuid::Uuid::from_bytes(owner_id.0);
        let owner_record = user_store
            .find_by_id(&owner_uuid.to_string())
            .await
            .map_err(|e| VerifierResolutionError::UserStore(e.to_string()))?;
        if let Some(owner_record) = &owner_record {
            if let Some(parent_id) = &owner_record.parent_user_id {
                if let Ok(parent_uuid) = uuid::Uuid::parse_str(parent_id) {
                    verifiers.push(AuthorizedVerifier {
                        user_id: ObjectId(*parent_uuid.as_bytes()),
                        reason: VerifierReason::TreeParent,
                    });
                }
            }
        }
    }

    // --- Phase C: active-loan widening (unaffected by escalation —
    // still applies regardless of whether this specific list has been
    // escalated, since a loan concerns the owner generally, not one
    // list's verification state) ---
    let rows = fetch_rows(staff_loan_repo_pool, organization_id, Some("staff_loan"))
        .await
        .map_err(|e| VerifierResolutionError::Projection(e.to_string()))?;
    for row in rows {
        let loan: StaffLoan = serde_json::from_value(row.state)
            .map_err(|e| VerifierResolutionError::Deserialization(e.to_string()))?;
        if loan.staff_user_id() != owner_id {
            continue;
        }
        if loan.grants_verification_authority_to(loan.real_owner_id()) {
            push_if_new(
                &mut verifiers,
                loan.real_owner_id(),
                VerifierReason::LoanRealOwner,
            );
        }
        if loan.grants_verification_authority_to(loan.borrowing_manager_id()) {
            push_if_new(
                &mut verifiers,
                loan.borrowing_manager_id(),
                VerifierReason::LoanBorrowingManager,
            );
        }
    }

    Ok(verifiers)
}

/// Whether `user_id` is authorized to verify a list owned by
/// `owner_id`. Convenience wrapper over `resolve_verifiers` for the
/// common "check one specific actor" call shape (e.g. a route handler
/// deciding whether to accept a `VerifyTodoList` command from the
/// caller), rather than every call site re-implementing the `.any(...)`
/// check. See `resolve_verifiers`'s doc comment for `escalated_to`.
pub async fn is_authorized_verifier(
    candidate: ObjectId,
    owner_id: ObjectId,
    organization_id: uuid::Uuid,
    user_store: &Arc<dyn UserStore>,
    staff_loan_repo_pool: &ProjectionPool,
    escalated_to: Option<ObjectId>,
) -> Result<bool, VerifierResolutionError> {
    let verifiers = resolve_verifiers(
        owner_id,
        organization_id,
        user_store,
        staff_loan_repo_pool,
        escalated_to,
    )
    .await?;
    Ok(verifiers.iter().any(|v| v.user_id == candidate))
}

/// Pushes `(user_id, reason)` only if `user_id` is not already present
/// — a user who is both the tree parent and a loan party (e.g. the real
/// owner requests a loan for their own report to a peer, then the loan
/// ends up naming themselves somehow — an unusual but not
/// domain-impossible configuration) should appear once, not twice, in
/// the returned list; `is_authorized_verifier`'s `.any()` check would
/// be correct either way, but a duplicate-free list is the more honest
/// contract for any caller that enumerates `AuthorizedVerifier`s
/// directly (e.g. to render "verifiable by: X, Y" in a UI).
fn push_if_new(verifiers: &mut Vec<AuthorizedVerifier>, user_id: ObjectId, reason: VerifierReason) {
    if !verifiers.iter().any(|v| v.user_id == user_id) {
        verifiers.push(AuthorizedVerifier { user_id, reason });
    }
}

#[cfg(test)]
mod tests {
    // Full integration coverage (a real UserStore + real staff_loan
    // rows in a real ProjectionPool) is exercised in
    // `tests::verifier_resolution_integration` at the crate-test level
    // (see `crates/bins/api-server/tests/`), not here — this module has
    // no meaningful unit-testable logic in isolation from those two
    // real dependencies (both `find_by_id` and `fetch_rows` are thin
    // wrappers over a live pool), so a mock-heavy unit test here would
    // mostly test the mocks, not this function's actual join logic.
}
