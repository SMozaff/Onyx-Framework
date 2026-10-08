//! OutboxStore port.
//! Source: Team Prompt 2 §3.3 (verbatim from Handover Document, unchanged
//! by any ruling). `ClaimedMessage` and `LeaseToken` per Architectural
//! Ruling (Team 2 Initiation) — referenced in §3.3/§5.1 but never defined
//! in the Handover Document.

use async_trait::async_trait;
use platform_kernel::Duration;
use query_application::OutboxId;
use query_application::OutboxMessage;

#[async_trait]
pub trait OutboxStore: Send + Sync {
    /// Claim up to `limit` unpublished messages for a given consumer group.
    /// Returns messages and a lease token. The lease prevents duplicate processing.
    async fn claim_unpublished(
        &self,
        consumer_id: &str,
        limit: usize,
    ) -> Result<Vec<ClaimedMessage>, OutboxError>;

    /// Mark a message as successfully published. This removes it from the pending queue.
    async fn mark_published(&self, outbox_id: OutboxId) -> Result<(), OutboxError>;

    /// Mark a message as failed (with retry count and next_attempt_at).
    async fn mark_failed(
        &self,
        outbox_id: OutboxId,
        error: &str,
        retry_after: Duration,
    ) -> Result<(), OutboxError>;

    /// Move a message to the dead-letter table (permanent failure).
    async fn dead_letter(&self, outbox_id: OutboxId, error: &str) -> Result<(), OutboxError>;

    /// Get the number of pending messages for monitoring.
    async fn pending_count(&self) -> Result<u64, OutboxError>;
}

/// A message claimed for processing, with its lease and current retry count.
/// Per Architectural Ruling (Team 2 Initiation): owned by `worker-application`.
#[derive(Debug, Clone)]
pub struct ClaimedMessage {
    pub outbox_id: OutboxId,
    pub message: OutboxMessage,
    pub lease_token: LeaseToken,
    pub retry_count: u32,
}

/// Opaque lease token proving exclusive claim over an outbox row for the
/// duration of the lease. Per Architectural Ruling (Team 2 Initiation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseToken(pub String);

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OutboxError {
    #[error("no messages to claim")]
    Empty,
    #[error("lease expired")]
    LeaseExpired,
    #[error("database error: {0}")]
    Database(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}
