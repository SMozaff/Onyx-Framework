//! Escalation-target resolution: given a `TodoList`/`TargetList`'s
//! *current* verifier, who does escalating it route to next?
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §4.1 and
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` E.1/E.2, confirmed by the
//! person directly on 2026-08-16: escalation routes to **the current
//! verifier's own parent — one hop up the same reporting-line tree
//! `verifier_resolution::resolve_verifiers` already walks**, not to a
//! manually-picked person and not straight to the Top-level Manager.
//!
//! # Re-escalation (chaining)
//! E.1 describes this as "step-by-step, not a jump" — a deliberate
//! choice of wording that implies a list can be escalated more than
//! once if the new authority is *also* stuck (e.g. Manager → Senior
//! Manager → Top-level Manager, one hop at a time). Design doc §4.0.2's
//! shared verification structure means `Escalated` must therefore be a
//! valid starting status for `Verify`/`Reject`/`Escalate` alike, not
//! just `Submitted`/`TeamLeaderPreChecked` — see the corresponding fix
//! in `todo_domain::aggregate`'s `decide()` guards, added alongside
//! this module after live testing surfaced the gap (a first escalation
//! worked, but the escalation target then couldn't actually verify the
//! list — `decide()` rejected `VerifyTodoList` from `Escalated`).
//!
//! This module therefore does **not** take the list's `owner` as its
//! starting point — it takes whichever user is *currently* the
//! authorized verifier (the tree parent on a first escalation, or the
//! prior `escalated_to` on a re-escalation), supplied by the caller,
//! and walks exactly one hop from there. The caller (`routes::command`)
//! is responsible for determining which of those two the "current
//! verifier" is for a given command — see
//! `resolve_todo_list_escalation_command`'s doc comment.
//!
//! # Which verifier is "the" one being escalated past, on a first
//! escalation?
//! `resolve_verifiers` can return more than one authorized verifier —
//! the tree parent, plus up to two staff-loan parties if the owner is
//! currently on an active loan (design doc §2.1's "either can verify"
//! rule). A first escalation specifically walks up from the **tree
//! parent**, not a loan party: the org tree is the durable, structural
//! chain of authority E.1 describes, while a staff loan is a temporary
//! overlay (design doc §2.1) that was never meant to extend or
//! redirect the permanent escalation ladder.
//!
//! # Chain termination
//! E.1: "Chain terminates at Top-level Manager... there is no
//! platform-owner rung above it to escalate into." If the current
//! verifier has no `parent_user_id`, escalation has nowhere to go —
//! `resolve_escalation_target` returns `Ok(None)`, a real, expected
//! outcome the caller must handle (e.g. by rejecting the escalation
//! command with a clear message), not an error.

use std::sync::Arc;

use platform_kernel::ObjectId;
use security_application::UserStore;

#[derive(Debug, thiserror::Error)]
pub enum EscalationResolutionError {
    #[error("user store lookup failed: {0}")]
    UserStore(String),
}

/// Resolves who escalating past `current_verifier_id` should route to:
/// that user's own `parent_user_id`, one hop up the tree. Returns
/// `Ok(None)` if `current_verifier_id` has no parent of their own
/// (chain terminates at Top-level Manager, E.1) or does not exist.
pub async fn resolve_escalation_target(
    current_verifier_id: ObjectId,
    user_store: &Arc<dyn UserStore>,
) -> Result<Option<ObjectId>, EscalationResolutionError> {
    let verifier_uuid = uuid::Uuid::from_bytes(current_verifier_id.0);
    let verifier_record = user_store
        .find_by_id(&verifier_uuid.to_string())
        .await
        .map_err(|e| EscalationResolutionError::UserStore(e.to_string()))?;
    let Some(verifier_record) = verifier_record else {
        return Ok(None);
    };
    let Some(escalation_target_id) = verifier_record.parent_user_id else {
        // The current verifier has no parent of their own -- chain
        // terminates here (E.1: "Chain terminates at Top-level
        // Manager").
        return Ok(None);
    };
    let Ok(escalation_target_uuid) = uuid::Uuid::parse_str(&escalation_target_id) else {
        return Ok(None);
    };

    Ok(Some(ObjectId(*escalation_target_uuid.as_bytes())))
}

#[cfg(test)]
mod tests {
    // Same rationale as `verifier_resolution::tests`'s empty module:
    // this function's only real logic is one `UserStore` lookup, so
    // meaningful coverage needs a real `UserStore` against a real tree,
    // exercised at the crate-test level (see
    // `crates/bins/api-server/tests/`), not mocked here.
}
