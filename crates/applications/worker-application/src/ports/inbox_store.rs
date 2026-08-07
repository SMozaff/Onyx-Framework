//! InboxStore port.
//! Source: Team Prompt 2 §3.4, verbatim from Handover Document, unchanged
//! by any ruling.

use async_trait::async_trait;
use platform_kernel::{EventId, Timestamp};

#[async_trait]
pub trait InboxStore: Send + Sync {
    /// Check if the given (consumer_id, event_id) has already been processed.
    async fn is_processed(&self, consumer_id: &str, event_id: &EventId)
        -> Result<bool, InboxError>;

    /// Record that this event has been processed by this consumer.
    async fn mark_processed(
        &self,
        consumer_id: &str,
        event_id: &EventId,
        processed_at: Timestamp,
    ) -> Result<(), InboxError>;

    /// Get the last processed event for a consumer (for cursor recovery).
    async fn last_processed(&self, consumer_id: &str) -> Result<Option<EventId>, InboxError>;
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InboxError {
    #[error("database error: {0}")]
    Database(String),
    #[error("event already processed")]
    AlreadyProcessed,
}
