//! IdempotencyStore port.
//! Per Architectural Ruling (Team 2 Kickoff, item 4): referenced in Team
//! Prompt 2 §6.1 ("server returns cached result for duplicate OperationId",
//! per Handover Document §16 delivery_guarantees.commands) but never
//! defined. Owned by `query-application` per the ruling's placement table.
//!
//! `result`/return type is `serde_json::Value` (a serialized `CommandResult`
//! -shaped payload), consistent with `UnitOfWork::register_idempotency_result`
//! per "Ruling — Team 1/Team 2 Interface Reconciliation".

use async_trait::async_trait;
use platform_kernel::OperationId;

#[async_trait]
pub trait IdempotencyStore: Send + Sync {
    /// Look up a previously-recorded result for this OperationId, if any.
    /// Used by the command handler (§6.1) to short-circuit duplicate
    /// command submissions and return the cached result instead of
    /// re-executing the command.
    async fn get(
        &self,
        operation_id: &OperationId,
    ) -> Result<Option<serde_json::Value>, IdempotencyError>;

    /// Record the result of a command execution against its OperationId.
    /// Per Handover §16: idempotency_key is OperationId; this write MUST
    /// happen in the same transaction as the aggregate/event/outbox writes
    /// (see UnitOfWork::register_idempotency_result and §6.2 Atomicity Rule).
    async fn put(
        &self,
        operation_id: OperationId,
        result: serde_json::Value,
    ) -> Result<(), IdempotencyError>;
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IdempotencyError {
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}
