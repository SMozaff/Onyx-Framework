//! The domain event envelope: the transport-level wrapper around every
//! event payload, carrying identity, versioning, causality, and audit
//! metadata.

use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
    ObjectVersion, OperationId, OrganizationId, ReplicaId, SchemaVersion, Timestamp, VectorClock,
};
use serde::{Deserialize, Serialize};

/// Wraps a domain event payload `E` with the full transport envelope
/// required for ordering, causal tracing, and audit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DomainEventEnvelope<E> {
    /// Unique identifier for this event instance.
    pub event_id: EventId,
    /// The event's type name (e.g. `"MissionPaused"`).
    pub event_type: String,
    /// The schema version this event payload was encoded with.
    pub schema_version: SchemaVersion,
    /// The aggregate this event was produced by.
    pub aggregate_ref: DomainObjectRef,
    /// The aggregate's version immediately after this event was applied.
    pub aggregate_version: ObjectVersion,
    /// The aggregate's lifecycle epoch immediately after this event.
    pub lifecycle_epoch: LifecycleEpoch,
    /// The aggregate's authority epoch immediately after this event.
    pub authority_epoch: AuthorityEpoch,
    /// The operation that produced this event.
    pub operation_id: OperationId,
    /// The actor whose command produced this event.
    pub actor: ActorContext,
    /// The logical time the underlying business fact occurred.
    pub occurred_at: Timestamp,
    /// The time this event was recorded by the system.
    pub recorded_at: Timestamp,
    /// The vector clock at the time this event was recorded.
    pub vector_clock: VectorClock,
    /// Correlates this event with the broader request/workflow it belongs
    /// to.
    pub correlation_id: CorrelationId,
    /// If this event was produced in response to a prior event, that
    /// event's identifier.
    pub causation_id: Option<EventId>,
    /// Audit and compliance metadata.
    pub audit_metadata: AuditMetadata,
    /// The event-specific payload.
    pub payload: E,
}

/// Audit and compliance metadata attached to every recorded event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditMetadata {
    /// The name of the service that recorded this event.
    pub source_service: String,
    /// The replica that recorded this event.
    pub source_replica: ReplicaId,
    /// An optional cryptographic integrity digest over the event payload.
    pub integrity_digest: Option<Vec<u8>>,
    /// The data classification level of this event's contents.
    pub classification: DataClassification,
    /// The tenant isolation key, used to enforce multi-tenant data
    /// separation at every downstream consumer.
    pub tenant_isolation_key: OrganizationId,
}

/// Data sensitivity classification for audit/compliance purposes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataClassification {
    /// Freely shareable.
    Public,
    /// Internal use only.
    Internal,
    /// Confidential; restricted distribution.
    Confidential,
    /// Highly restricted; strictest handling requirements.
    Restricted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use platform_kernel::ObjectId;

    fn sample_envelope() -> DomainEventEnvelope<serde_json::Value> {
        let org = ObjectId::new_random();
        DomainEventEnvelope {
            event_id: EventId::new_random(),
            event_type: "TestEvent".to_string(),
            schema_version: SchemaVersion::new("1.0.0"),
            aggregate_ref: DomainObjectRef {
                id: ObjectId::new_random(),
                r#type: "mission".to_string(),
                organization_id: org,
            },
            aggregate_version: ObjectVersion(1),
            lifecycle_epoch: LifecycleEpoch(0),
            authority_epoch: AuthorityEpoch(0),
            operation_id: OperationId::new_random(),
            actor: ActorContext {
                user_id: ObjectId::new_random(),
                device_id: ObjectId::new_random(),
                organization_id: org,
            },
            occurred_at: Timestamp::from_nanos(0),
            recorded_at: Timestamp::from_nanos(1),
            vector_clock: VectorClock::new(),
            correlation_id: CorrelationId::new_random(),
            causation_id: None,
            audit_metadata: AuditMetadata {
                source_service: "mission-domain".to_string(),
                source_replica: ReplicaId::new_random(),
                integrity_digest: None,
                classification: DataClassification::Internal,
                tenant_isolation_key: org,
            },
            payload: serde_json::json!({"key": "value"}),
        }
    }

    #[test]
    fn event_envelope_serde_roundtrips() {
        let envelope = sample_envelope();
        let json = serde_json::to_string(&envelope).unwrap();
        let back: DomainEventEnvelope<serde_json::Value> = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope.event_type, back.event_type);
        assert_eq!(envelope.payload, back.payload);
    }
}
