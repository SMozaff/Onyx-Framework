use platform_contracts::{AuditMetadata, DataClassification, DomainEventEnvelope};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
    ObjectId, ObjectVersion, OperationId, OrganizationId, ReplicaId, SchemaVersion, Timestamp,
    VectorClock,
};
use synchronization_domain::{
    ConflictResolution, ConflictStatus, FieldMetadata, FieldShape, SynchronizationSession,
};

use super::test_harness::PostgresHarness;

fn operation(
    aggregate_id: ObjectId,
    org: OrganizationId,
    replica: ReplicaId,
    event_type: &str,
) -> DomainEventEnvelope<serde_json::Value> {
    let mut vector_clock = VectorClock::new();
    vector_clock.increment(replica);
    DomainEventEnvelope {
        event_id: EventId::new_random(),
        event_type: event_type.to_string(),
        schema_version: SchemaVersion::new("1.0"),
        aggregate_ref: DomainObjectRef {
            id: aggregate_id,
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
        occurred_at: Timestamp::now(),
        recorded_at: Timestamp::now(),
        vector_clock,
        correlation_id: CorrelationId::new_random(),
        causation_id: None,
        audit_metadata: AuditMetadata {
            source_service: "team8-e2e".to_string(),
            source_replica: replica,
            integrity_digest: None,
            classification: DataClassification::Internal,
            tenant_isolation_key: org,
        },
        payload: serde_json::json!({"field": "status"}),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn journey_3_conflict_resolution() -> anyhow::Result<()> {
    let postgres = PostgresHarness::start().await?;
    let org = OrganizationId::new_random();
    let aggregate_id = ObjectId::new_random();
    let local = ReplicaId::new_random();
    let remote = ReplicaId::new_random();
    let mut session =
        SynchronizationSession::new(local, remote, org).with_field_metadata(vec![FieldMetadata {
            name: "status".to_string(),
            shape: FieldShape::AuthorityControlled,
            is_authority_controlled: true,
            is_crdt_eligible: false,
        }]);
    session.exchange_operations(vec![operation(
        aggregate_id,
        org,
        local,
        "MissionActivated",
    )])?;
    session.exchange_operations(vec![operation(aggregate_id, org, remote, "MissionPaused")])?;
    let mut conflicts = session.reconcile()?;
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].status, ConflictStatus::Open);
    conflicts[0].resolve(ConflictResolution::LocalWins);
    assert_eq!(conflicts[0].status, ConflictStatus::Resolved);
    postgres
        .record("journey-3-conflict-resolution", "passed")
        .await?;
    Ok(())
}
