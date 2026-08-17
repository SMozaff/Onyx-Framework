//! Shared notification aggregate contract.
//!
//! Notifications are replicated aggregate state addressed to one recipient.
//! Creation is performed by upstream workflow producers or synchronized into a
//! client replica; this crate owns the acknowledgement transition so every
//! client uses one state and event shape.

use chrono::{DateTime, SecondsFormat, Utc};
use platform_contracts::{AggregateRoot, DecisionContext};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectId, ObjectVersion};
use serde::{Deserialize, Serialize};

/// A notification stored in a client's local replica.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationAggregate {
    pub id: ObjectId,
    pub public_id: String,
    pub title: String,
    pub message: String,
    pub priority: String,
    pub status: String,
    /// The user this notification is addressed to. Legacy records deserialize
    /// as `None` and are intentionally excluded from recipient inbox queries.
    #[serde(default)]
    pub recipient_id: Option<String>,
    pub source_id: String,
    pub source_type: String,
    pub created_at: String,
    pub acknowledged_at: Option<String>,
    pub version: ObjectVersion,
    pub lifecycle_epoch: LifecycleEpoch,
    pub authority_epoch: AuthorityEpoch,
}

/// Commands supported by an existing notification aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationCommand {
    Acknowledge,
}

/// Events emitted by notification decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationEvent {
    Acknowledged { acknowledged_at: String },
}

/// Notification decision failures.
#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("notification already acknowledged")]
    AlreadyAcknowledged,
}

impl AggregateRoot for NotificationAggregate {
    type Id = ObjectId;
    type Command = NotificationCommand;
    type Event = NotificationEvent;
    type Error = NotificationError;

    fn id(&self) -> &Self::Id {
        &self.id
    }

    fn version(&self) -> ObjectVersion {
        self.version
    }

    fn lifecycle_epoch(&self) -> LifecycleEpoch {
        self.lifecycle_epoch
    }

    fn authority_epoch(&self) -> AuthorityEpoch {
        self.authority_epoch
    }

    fn decide(
        &self,
        command: Self::Command,
        context: &DecisionContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            NotificationCommand::Acknowledge if self.status == "acknowledged" => {
                Err(NotificationError::AlreadyAcknowledged)
            }
            NotificationCommand::Acknowledge => Ok(vec![NotificationEvent::Acknowledged {
                acknowledged_at: timestamp_to_iso(context.trusted_now.0),
            }]),
        }
    }

    fn apply(&mut self, event: &Self::Event) {
        match event {
            NotificationEvent::Acknowledged { acknowledged_at } => {
                self.status = "acknowledged".to_string();
                self.acknowledged_at = Some(acknowledged_at.clone());
                self.version = self.version.next();
            }
        }
    }
}

fn timestamp_to_iso(nanos: u64) -> String {
    let seconds = (nanos / 1_000_000_000) as i64;
    let subsec_nanos = (nanos % 1_000_000_000) as u32;
    DateTime::<Utc>::from_timestamp(seconds, subsec_nanos)
        .unwrap_or(DateTime::<Utc>::UNIX_EPOCH)
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_contracts::{DecisionContext, IdGenerator};
    use platform_kernel::{
        ActorContext, EventId, OperationId, OrganizationId, PolicyDecisionSet, Timestamp,
        VerifiedAuthority,
    };

    struct TestIds;
    impl IdGenerator for TestIds {
        fn generate_object_id(&self) -> ObjectId {
            ObjectId::new_random()
        }
        fn generate_operation_id(&self) -> OperationId {
            OperationId::new_random()
        }
        fn generate_event_id(&self) -> EventId {
            EventId::new_random()
        }
    }

    fn sample() -> NotificationAggregate {
        NotificationAggregate {
            id: ObjectId::new_random(),
            public_id: "notice-1".to_string(),
            title: "Approval needed".to_string(),
            message: "Please review.".to_string(),
            priority: "normal".to_string(),
            status: "unacknowledged".to_string(),
            recipient_id: Some(ObjectId::new_random().to_string()),
            source_id: "source-1".to_string(),
            source_type: "task".to_string(),
            created_at: "2026-08-17T00:00:00.000Z".to_string(),
            acknowledged_at: None,
            version: ObjectVersion::INITIAL,
            lifecycle_epoch: LifecycleEpoch::INITIAL,
            authority_epoch: AuthorityEpoch::INITIAL,
        }
    }

    fn context() -> DecisionContext {
        DecisionContext {
            actor: ActorContext {
                user_id: ObjectId::new_random(),
                device_id: ObjectId::new_random(),
                organization_id: OrganizationId::new_random(),
            },
            authority: VerifiedAuthority,
            trusted_now: Timestamp::from_nanos(1_700_000_000_000_000_000),
            policy_outcomes: PolicyDecisionSet,
            generated_id_generator: Box::new(TestIds),
        }
    }

    #[test]
    fn acknowledgement_marks_the_notification_read() {
        let mut notification = sample();
        let events = notification
            .decide(NotificationCommand::Acknowledge, &context())
            .unwrap();
        notification.apply(&events[0]);
        assert_eq!(notification.status, "acknowledged");
        assert!(notification.acknowledged_at.is_some());
        assert_eq!(notification.version, ObjectVersion::INITIAL.next());
    }

    #[test]
    fn acknowledgement_is_not_repeated() {
        let mut notification = sample();
        let events = notification
            .decide(NotificationCommand::Acknowledge, &context())
            .unwrap();
        notification.apply(&events[0]);
        assert!(matches!(
            notification.decide(NotificationCommand::Acknowledge, &context()),
            Err(NotificationError::AlreadyAcknowledged)
        ));
    }
}
