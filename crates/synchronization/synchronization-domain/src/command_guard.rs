//! Command Rejection Guard (for ConflictPending).
//! Source: Team Prompt 3 §3.14, rebuilt against the real
//! `platform_contracts_ext::AggregateRoot::conflict_pending()` (Ruling Q2)
//! and the real, generic `CommandEnvelope<C>`.

use platform_contracts::CommandEnvelope;
use platform_contracts_ext::{AggregateRoot, ConflictPendingMarker};
use platform_kernel::ConflictId;

/// Errors returned by the conflict-pending guard.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardError {
    /// The aggregate has an unresolved conflict and cannot be mutated.
    #[error("conflict pending: {conflict_id:?} - cannot mutate while conflict unresolved")]
    ConflictPending {
        /// The unresolved conflict blocking this command.
        conflict_id: ConflictId,
    },
}

/// If the aggregate has a conflict pending, reject all mutating commands.
/// Only allow read/query-style commands (by convention, a `command_type`
/// prefixed `"query."`) through.
pub fn check_conflict_pending<A: AggregateRoot, C>(
    aggregate: &A,
    envelope: &CommandEnvelope<C>,
) -> Result<(), GuardError> {
    if let Some(ConflictPendingMarker { conflict_id, .. }) = aggregate.conflict_pending() {
        if !envelope.command_type.starts_with("query.") {
            return Err(GuardError::ConflictPending { conflict_id });
        }
    }
    Ok(())
}
