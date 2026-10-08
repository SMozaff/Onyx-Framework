//! EventPublisher port.
//! Source: Team Prompt 2 §3.5, verbatim from Handover Document, unchanged
//! by any ruling.

use async_trait::async_trait;
use query_application::OutboxMessage;

#[async_trait]
pub trait EventPublisher: Send + Sync {
    /// Publish a message to the broker (NATS, Kafka, or embedded channel).
    /// The caller already holds the outbox row; this is only the network call.
    async fn publish(&self, message: &OutboxMessage) -> Result<(), PublishError>;
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PublishError {
    #[error("transient network error: {0}")]
    Transient(String),
    #[error("permanent error: {0}")]
    Permanent(String),
    #[error("message too large")]
    MessageTooLarge,
    #[error("broker unavailable")]
    BrokerUnavailable,
}

impl PublishError {
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            PublishError::Transient(_) | PublishError::BrokerUnavailable
        )
    }
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            PublishError::Permanent(_) | PublishError::MessageTooLarge
        )
    }
}
