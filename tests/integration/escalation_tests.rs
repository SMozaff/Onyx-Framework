//! Escalation Under Partition Test.
//! Source: Team Prompt 3 §6.4, rebuilt against the real, event-sourced
//! `EscalationService` (Architectural Ruling Q3) — see
//! `synchronization-domain::escalation`'s module doc comment for exactly
//! what changed and why.

use std::sync::Arc;

use platform_contracts::{AuditMetadata, DataClassification, DomainEventEnvelope};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
    ObjectId, ObjectVersion, OperationId, OrganizationId, ReplicaId, SchemaVersion, Timestamp,
    VectorClock,
};
use sync_test_utils::MockRepository;
use synchronization_domain::{ConflictRecord, EscalationService, PolicyConfig};

fn dummy_envelope(
    aggregate_id: ObjectId,
    org_id: OrganizationId,
) -> DomainEventEnvelope<serde_json::Value> {
    DomainEventEnvelope {
        event_id: EventId::new_random(),
        event_type: "TestEvent".to_string(),
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
        occurred_at: Timestamp::now(),
        recorded_at: Timestamp::now(),
        vector_clock: VectorClock::new(),
        correlation_id: CorrelationId::new_random(),
        causation_id: None,
        audit_metadata: AuditMetadata {
            source_service: "test".to_string(),
            source_replica: ReplicaId::new_random(),
            integrity_digest: None,
            classification: DataClassification::Internal,
            tenant_isolation_key: org_id,
        },
        payload: serde_json::json!({}),
    }
}

#[tokio::test]
async fn escalation_works_without_network() {
    let repo = Arc::new(MockRepository::new());
    let config = PolicyConfig {
        escalation_window_nanos: 3600 * 1_000_000_000,
    };
    let service = EscalationService::new(repo.clone(), repo.clone(), config);

    let aggregate_id = ObjectId::new_random();
    let org_id = OrganizationId::new_random();
    let mut conflict = ConflictRecord::new(
        aggregate_id,
        "status".to_string(),
        dummy_envelope(aggregate_id, org_id),
        dummy_envelope(aggregate_id, org_id),
        org_id,
    );
    conflict.created_at =
        Timestamp::from_nanos(conflict.created_at.0.saturating_sub(7200 * 1_000_000_000)); // 2 hours ago

    // Escalate. No network call anywhere in this path — the only I/O is
    // the durable, transactional MockRepository write (mirroring the real
    // Repository/UnitOfWorkFactory pipeline).
    service.escalate_conflict(&conflict).await.unwrap();

    // Verify the event was durably recorded.
    let events = repo.committed_events();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0]["event_type"].as_str(),
        Some("synchronization.ConflictEscalationRequested")
    );
}

/// Additional coverage: `scan_and_escalate` should find conflicts older
/// than the escalation window via `ConflictRepository` and escalate them,
/// while leaving fresh conflicts alone.
#[tokio::test]
async fn scan_and_escalate_only_escalates_expired_conflicts() {
    use sync_test_utils::MockConflictRepository;

    let repo = Arc::new(MockRepository::new());
    let config = PolicyConfig {
        escalation_window_nanos: 3600 * 1_000_000_000,
    };
    let service = EscalationService::new(repo.clone(), repo.clone(), config);

    let aggregate_id = ObjectId::new_random();
    let org_id = OrganizationId::new_random();

    let mut old_conflict = ConflictRecord::new(
        aggregate_id,
        "status".to_string(),
        dummy_envelope(aggregate_id, org_id),
        dummy_envelope(aggregate_id, org_id),
        org_id,
    );
    old_conflict.created_at = Timestamp::from_nanos(
        old_conflict
            .created_at
            .0
            .saturating_sub(7200 * 1_000_000_000),
    );

    let fresh_conflict = ConflictRecord::new(
        aggregate_id,
        "status".to_string(),
        dummy_envelope(aggregate_id, org_id),
        dummy_envelope(aggregate_id, org_id),
        org_id,
    );
    // fresh_conflict.created_at defaults to now() via ::new(), well within
    // the 1-hour window.

    let conflict_repo = Arc::new(MockConflictRepository::with_conflicts(vec![
        old_conflict,
        fresh_conflict,
    ]));

    let escalated_count = service.scan_and_escalate(conflict_repo).await.unwrap();
    assert_eq!(escalated_count, 1);
    assert_eq!(repo.committed_events().len(), 1);
}
