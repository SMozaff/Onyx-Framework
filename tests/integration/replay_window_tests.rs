//! No Fixed Window Test.
//! Source: §8 acceptance criteria row "No Fixed Window" names
//! `replay_window_tests::old_operation_still_reconciled`; the test body is
//! never shown anywhere in §6.
//!
//! What this proves: reconciliation is gated ONLY on causal relation
//! (`VectorClock::compare`), never on how much wall-clock time has elapsed
//! since an operation's `occurred_at`.

use platform_contracts::{AuditMetadata, DataClassification, DomainEventEnvelope};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CausalRelation, CorrelationId, DomainObjectRef, EventId,
    LifecycleEpoch, ObjectId, ObjectVersion, OperationId, OrganizationId, ReplicaId, SchemaVersion,
    Timestamp, VectorClock,
};
use synchronization_domain::SynchronizationSession;

#[test]
fn old_operation_still_reconciled() {
    let replica_a = ReplicaId::new_random();
    let replica_b = ReplicaId::new_random();
    let org_id = OrganizationId::new_random();
    let aggregate_id = ObjectId::new_random();

    let mut session = SynchronizationSession::new(replica_a, replica_b, org_id);

    let mut clock = VectorClock::new();
    clock.increment(replica_b);

    // An operation whose occurred_at is deliberately as old as
    // representable — well beyond any plausible fixed sync window — but
    // whose causal position (empty session frontier -> this op's clock) is
    // squarely "session frontier is BEFORE this op", i.e. genuinely new
    // information the session hasn't seen yet.
    let ancient_but_causally_new_op = DomainEventEnvelope {
        event_id: EventId::new_random(),
        event_type: "AncientEvent".to_string(),
        schema_version: SchemaVersion::new("1.0"),
        aggregate_ref: DomainObjectRef {
            id: aggregate_id,
            r#type: "mission".to_string(),
            organization_id: org_id,
        },
        aggregate_version: ObjectVersion(1),
        lifecycle_epoch: LifecycleEpoch(0),
        authority_epoch: AuthorityEpoch(0),
        operation_id: OperationId::new_random(),
        actor: ActorContext {
            user_id: ObjectId::new_random(),
            device_id: ObjectId::new_random(),
            organization_id: org_id,
        },
        occurred_at: Timestamp::from_nanos(0), // Unix epoch: as old as representable
        recorded_at: Timestamp::from_nanos(0),
        vector_clock: clock,
        correlation_id: CorrelationId::new_random(),
        causation_id: None,
        audit_metadata: AuditMetadata {
            source_service: "test".to_string(),
            source_replica: replica_b,
            integrity_digest: None,
            classification: DataClassification::Internal,
            tenant_isolation_key: org_id,
        },
        payload: serde_json::json!({ "field": "notes" }),
    };

    let outcomes = session
        .exchange_operations(vec![ancient_but_causally_new_op])
        .unwrap();

    // Reconciliation was attempted (the op was buffered and produced an
    // outcome), regardless of its age — proving there is no fixed replay
    // window silently dropping it.
    assert_eq!(outcomes.len(), 1);
    assert_ne!(
        session.vector_clock().compare(&VectorClock::new()),
        CausalRelation::Equal
    );
}
