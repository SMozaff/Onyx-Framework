//! `EventBus` — broadcasts committed domain events (JSON-encoded
//! `platform_contracts::DomainEventEnvelope<E>`, produced by
//! `api_server::handle_command`'s Step 8) to UI subscribers.
//!
//! # Provenance
//! Team Prompt 5 §3.2 (`subscribe_events`) references `EventFilter` and an
//! `event_bus.subscribe(filter)` call by name only; neither `EventFilter`
//! nor `EventBus` is defined anywhere in Increments 1–4 (verified by
//! exhaustive grep — see R1 in `DECISIONS.md`). This is new
//! infrastructure, not a wrapper around an existing type.
//!
//! Desktop-shell forwards every event from `subscribe()`'s receiver to a
//! Tauri `app_handle.emit_all("onyx:event", ...)` call; mobile-core
//! forwards them to the FFI `callback` registered via
//! `mobile_core_subscribe_events`. Filtering (`EventFilter`) is applied at
//! the point of forwarding, not inside the channel itself, so every
//! subscriber sees the same underlying event stream and the filter is
//! just a client-side predicate.

use platform_kernel::OrganizationId;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use uuid::Uuid;

/// A single committed domain event, JSON-encoded (mirrors
/// `platform_contracts::DomainEventEnvelope<serde_json::Value>` — the
/// `payload` field is left as `Value` since the bus is generic over every
/// aggregate's event type).
pub type EventEnvelopeJson = serde_json::Value;

/// A predicate over the event stream. Not defined by any prior increment
/// (see module doc comment) — deliberately minimal: filters by
/// organization (tenant isolation, required — see
/// `platform_kernel::OrganizationId` usage throughout Increment 1/2) and
/// optionally by event type name. Extend as concrete UI needs emerge.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventFilter {
    pub organization_id: OrganizationId,
    /// If `Some`, only events whose `event_type` is in this list are
    /// delivered. `None` means "all event types for this organization".
    pub event_types: Option<Vec<String>>,
}

impl EventFilter {
    fn matches(&self, envelope: &EventEnvelopeJson) -> bool {
        let Some(org) = envelope.get("audit_metadata").and_then(|m| m.get("tenant_isolation_key")) else {
            return false;
        };
        let Ok(org_id) = serde_json::from_value::<OrganizationId>(org.clone()) else {
            return false;
        };
        if org_id != self.organization_id {
            return false;
        }
        match &self.event_types {
            None => true,
            Some(types) => envelope
                .get("event_type")
                .and_then(|t| t.as_str())
                .map(|t| types.iter().any(|allowed| allowed == t))
                .unwrap_or(false),
        }
    }
}

/// Identifies one active subscription. Returned by `subscribe()` so the
/// Tauri command / FFI function has something to hand back to the caller,
/// per Team Prompt 5 §3.2's `subscribe_events -> EventStreamId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventStreamId(pub Uuid);

impl EventStreamId {
    fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// The shared event bus. One instance per client process, held in
/// `AppState`. Cloning is cheap (wraps a `broadcast::Sender`).
#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<EventEnvelopeJson>,
}

/// A live subscription: an id (for cancellation bookkeeping by the caller)
/// and a filtered receiver stream.
pub struct Subscription {
    pub id: EventStreamId,
    receiver: broadcast::Receiver<EventEnvelopeJson>,
    filter: EventFilter,
}

impl Subscription {
    /// Await the next event matching this subscription's filter,
    /// discarding non-matching events and lag notifications transparently.
    /// Returns `None` only if the underlying bus was dropped (process
    /// shutting down).
    pub async fn recv(&mut self) -> Option<EventEnvelopeJson> {
        loop {
            match self.receiver.recv().await {
                Ok(envelope) => {
                    if self.filter.matches(&envelope) {
                        return Some(envelope);
                    }
                    // Doesn't match this subscription's filter; keep waiting.
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    tracing::warn!(skipped, "event subscriber lagged; some events were dropped");
                    // Keep waiting for the next event rather than surfacing
                    // the lag as a stream-ending error — a UI missing a few
                    // intermediate events is recoverable (it will still
                    // observe eventually-consistent final state via the
                    // next query), unlike ending the subscription outright.
                }
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

impl EventBus {
    /// `capacity` bounds how many events a slow subscriber can lag behind
    /// before older events are dropped for it (see `Subscription::recv`'s
    /// `Lagged` handling). 1024 is a starting point, not a frozen contract
    /// value — no prior increment specifies one.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    /// Publish a committed event to all subscribers.
    ///
    /// # Where events actually come from (design note, not a frozen
    /// contract — flagged for review)
    /// `api_server::handle_command`'s return value (`CommandResult`) is a
    /// `{"success", "operation_id", "new_version", "event_count"}` summary
    /// — it does **not** include the committed event envelopes themselves;
    /// those are written straight to the repository/outbox inside
    /// `handle_command`, never handed back to the caller. So `publish()`
    /// cannot be called directly from `CommandRegistry::dispatch`'s return
    /// path. The composing client instead runs a small local "outbox pump"
    /// task (see `sync_agent.rs`'s `run_outbox_pump`) that polls the
    /// client's own local `OutboxStore` (Increment 2,
    /// `worker_application::OutboxStore::claim_unpublished`) — the same
    /// mechanism the real Increment 2 worker uses for cross-network
    /// delivery — and calls `publish()` for each claimed message before
    /// marking it published locally. This reuses real, delivered
    /// Increment 2 infrastructure rather than inventing a new one, but is
    /// a genuine design choice (not a mechanical signature fix) and is
    /// flagged as such.
    pub fn publish(&self, envelope: EventEnvelopeJson) {
        // A `send` error here means there are currently no subscribers;
        // that's a normal, expected state (e.g. at startup before the UI
        // has subscribed), not a failure.
        let _ = self.sender.send(envelope);
    }

    /// Subscribe to the event stream, filtered per `filter`.
    pub fn subscribe(&self, filter: EventFilter) -> Subscription {
        Subscription {
            id: EventStreamId::new(),
            receiver: self.sender.subscribe(),
            filter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_kernel::ObjectId;

    /// Builds a JSON value matching the real shape
    /// `api_server::handle_command`'s Step 8 produces for
    /// `DomainEventEnvelope`'s `audit_metadata.tenant_isolation_key`
    /// (populated from `organization_id`) and `event_type` fields — the
    /// two fields `EventFilter::matches` actually reads.
    fn sample_event(org: OrganizationId, event_type: &str) -> EventEnvelopeJson {
        serde_json::json!({
            "event_id": ObjectId::new_random(),
            "event_type": event_type,
            "audit_metadata": {
                "source_service": "test",
                "source_replica": ObjectId::new_random(),
                "integrity_digest": null,
                "classification": "Internal",
                "tenant_isolation_key": org,
            },
        })
    }

    #[tokio::test]
    async fn subscriber_receives_matching_event() {
        let bus = EventBus::new(16);
        let org = OrganizationId::new_random();
        let mut sub = bus.subscribe(EventFilter {
            organization_id: org,
            event_types: None,
        });

        bus.publish(sample_event(org, "MissionCreated"));

        let received = tokio::time::timeout(std::time::Duration::from_secs(1), sub.recv())
            .await
            .expect("recv timed out")
            .expect("subscription closed unexpectedly");
        assert_eq!(received["event_type"], serde_json::json!("MissionCreated"));
    }

    #[tokio::test]
    async fn subscriber_does_not_receive_event_from_a_different_organization() {
        let bus = EventBus::new(16);
        let my_org = OrganizationId::new_random();
        let other_org = OrganizationId::new_random();
        let mut sub = bus.subscribe(EventFilter {
            organization_id: my_org,
            event_types: None,
        });

        bus.publish(sample_event(other_org, "MissionCreated"));
        // Publish a second, matching event so recv() has something to
        // eventually return — proving the first (wrong-org) event was
        // filtered out rather than the test merely timing out.
        bus.publish(sample_event(my_org, "MissionCreated"));

        let received = tokio::time::timeout(std::time::Duration::from_secs(1), sub.recv())
            .await
            .expect("recv timed out")
            .expect("subscription closed unexpectedly");
        // If org filtering were broken, this would be the other_org event
        // (published first); confirm we skipped straight to the matching one.
        assert_eq!(received["audit_metadata"]["tenant_isolation_key"], serde_json::to_value(my_org).unwrap());
    }

    #[tokio::test]
    async fn subscriber_with_event_type_filter_only_receives_listed_types() {
        let bus = EventBus::new(16);
        let org = OrganizationId::new_random();
        let mut sub = bus.subscribe(EventFilter {
            organization_id: org,
            event_types: Some(vec!["MissionPaused".to_string()]),
        });

        bus.publish(sample_event(org, "MissionCreated")); // filtered out
        bus.publish(sample_event(org, "MissionPaused")); // delivered

        let received = tokio::time::timeout(std::time::Duration::from_secs(1), sub.recv())
            .await
            .expect("recv timed out")
            .expect("subscription closed unexpectedly");
        assert_eq!(received["event_type"], serde_json::json!("MissionPaused"));
    }

    #[tokio::test]
    async fn multiple_subscribers_each_receive_the_same_event() {
        let bus = EventBus::new(16);
        let org = OrganizationId::new_random();
        let mut sub_a = bus.subscribe(EventFilter {
            organization_id: org,
            event_types: None,
        });
        let mut sub_b = bus.subscribe(EventFilter {
            organization_id: org,
            event_types: None,
        });

        bus.publish(sample_event(org, "TaskCreated"));

        let a = tokio::time::timeout(std::time::Duration::from_secs(1), sub_a.recv())
            .await
            .unwrap()
            .unwrap();
        let b = tokio::time::timeout(std::time::Duration::from_secs(1), sub_b.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(a["event_type"], b["event_type"]);
    }

    #[tokio::test]
    async fn publish_with_no_subscribers_does_not_panic_or_error() {
        let bus = EventBus::new(16);
        // No subscribe() call at all; publish() must not panic (documented
        // "no subscribers yet" as a normal state, not a failure).
        bus.publish(sample_event(OrganizationId::new_random(), "MissionCreated"));
    }

    #[test]
    fn distinct_subscriptions_get_distinct_stream_ids() {
        let bus = EventBus::new(16);
        let org = OrganizationId::new_random();
        let sub_a = bus.subscribe(EventFilter {
            organization_id: org,
            event_types: None,
        });
        let sub_b = bus.subscribe(EventFilter {
            organization_id: org,
            event_types: None,
        });
        assert_ne!(sub_a.id, sub_b.id);
    }
}
