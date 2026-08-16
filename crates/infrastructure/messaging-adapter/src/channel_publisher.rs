//! In-process broadcast channel `EventPublisher`. Suitable for local
//! development, desktop, and integration tests. Replace with a NATS or
//! Kafka adapter in production deployments (DECISIONS.md item M1).

use async_trait::async_trait;
use query_application::OutboxMessage;
use tokio::sync::broadcast;
use worker_application::{EventPublisher, PublishError};

pub type ChannelEventReceiver = broadcast::Receiver<OutboxMessage>;

const DEFAULT_CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct ChannelEventPublisher {
    sender: broadcast::Sender<OutboxMessage>,
}

impl ChannelEventPublisher {
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(DEFAULT_CAPACITY);
        Self { sender }
    }

    pub fn new_with_capacity(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Subscribe to published events. Useful for tests and local consumers.
    pub fn subscribe(&self) -> ChannelEventReceiver {
        self.sender.subscribe()
    }
}

impl Default for ChannelEventPublisher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventPublisher for ChannelEventPublisher {
    async fn publish(&self, message: &OutboxMessage) -> Result<(), PublishError> {
        match self.sender.send(message.clone()) {
            Ok(_) => {
                tracing::debug!(
                    event_type = %message.event_type,
                    aggregate_id = %message.aggregate_id,
                    "published event to channel"
                );
                Ok(())
            }
            // send() only errors if there are no active receivers. Per the
            // Handover Document §16 delivery_guarantees, the outbox relay
            // guarantees delivery only to the broker — a missing subscriber
            // is not a transport error (the message is already persisted in
            // the outbox; if the subscriber restarts, it can replay from the
            // outbox). Treat as transient (the relay will retry; when the
            // subscriber reconnects, there will be a receiver). See
            // DECISIONS.md item M2.
            Err(_) => Err(PublishError::Transient(
                "no active subscribers on in-process channel (transient — subscriber may reconnect)".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_kernel::{EventId, ObjectId, Timestamp, VectorClock};

    fn sample_message() -> OutboxMessage {
        OutboxMessage {
            event_id: EventId([0u8; 16]),
            event_type: "test.Event".to_string(),
            aggregate_id: "test-id".to_string(),
            organization_id: ObjectId([0u8; 16]),
            payload: b"{}".to_vec(),
            vector_clock: VectorClock::new(),
            occurred_at: Timestamp::now(),
        }
    }

    #[tokio::test]
    async fn publish_delivers_to_subscriber() {
        let publisher = ChannelEventPublisher::new();
        let mut receiver = publisher.subscribe();
        let msg = sample_message();
        publisher.publish(&msg).await.unwrap();
        let received = receiver.recv().await.unwrap();
        assert_eq!(received.event_type, "test.Event");
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_returns_transient_error() {
        let publisher = ChannelEventPublisher::new();
        // No subscriber — no clone kept
        let result = publisher.publish(&sample_message()).await;
        assert!(matches!(result, Err(PublishError::Transient(_))));
    }
}
