//! SynchronizationSession Resumable Test.
//! Source: §8 acceptance criteria row "SynchronizationSession Resumable"
//! names `session_tests::session_resumes_after_interruption`; the test
//! body is never shown anywhere in §6.
//!
//! What this proves: pausing a session mid-Exchange (e.g. the OS suspends
//! a mobile app) and resuming it later produces the same final
//! vector-clock/reconciled-aggregate state as running the same operations
//! through an uninterrupted session.

use platform_contracts::{AuditMetadata, DataClassification, DomainEventEnvelope};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
    ObjectId, ObjectVersion, OperationId, OrganizationId, ReplicaId, SchemaVersion, Timestamp,
    VectorClock,
};
use synchronization_domain::{SessionState, SynchronizationSession};

fn make_op(
    aggregate_id: ObjectId,
    org_id: OrganizationId,
    replica: ReplicaId,
    seq: u64,
) -> DomainEventEnvelope<serde_json::Value> {
    let mut clock = VectorClock::new();
    for _ in 0..seq {
        clock.increment(replica);
    }
    DomainEventEnvelope {
        event_id: EventId::new_random(),
        event_type: "TestEvent".to_string(),
        schema_version: SchemaVersion::new("1.0"),
        aggregate_ref: DomainObjectRef {
            id: aggregate_id,
            r#type: "mission".to_string(),
            organization_id: org_id,
        },
        aggregate_version: ObjectVersion(seq),
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
        vector_clock: clock,
        correlation_id: CorrelationId::new_random(),
        causation_id: None,
        audit_metadata: AuditMetadata {
            source_service: "test".to_string(),
            source_replica: replica,
            integrity_digest: None,
            classification: DataClassification::Internal,
            tenant_isolation_key: org_id,
        },
        payload: serde_json::json!({ "field": "notes" }),
    }
}

#[test]
fn session_resumes_after_interruption() {
    let replica_a = ReplicaId::new_random();
    let replica_b = ReplicaId::new_random();
    let org_id = OrganizationId::new_random();
    let aggregate_id = ObjectId::new_random();

    let op1 = make_op(aggregate_id, org_id, replica_b, 1);
    let op2 = make_op(aggregate_id, org_id, replica_b, 2);

    // --- Uninterrupted run ---
    let mut uninterrupted = SynchronizationSession::new(replica_a, replica_b, org_id);
    uninterrupted
        .exchange_operations(vec![op1.clone(), op2.clone()])
        .unwrap();
    let uninterrupted_result = uninterrupted.complete();

    // --- Interrupted run: pause between the two operations, then resume ---
    let mut interrupted = SynchronizationSession::new(replica_a, replica_b, org_id);
    interrupted.exchange_operations(vec![op1]).unwrap();

    let cursor = interrupted.pause().unwrap();
    assert_eq!(interrupted.state(), SessionState::Paused);

    interrupted.resume().unwrap();
    assert_eq!(interrupted.state(), SessionState::Active);
    // pause() must hand back a cursor the caller could persist and later
    // use to reconstruct/resume the session from durable storage.
    let _ = cursor;

    interrupted.exchange_operations(vec![op2]).unwrap();
    let interrupted_result = interrupted.complete();

    assert_eq!(uninterrupted_result, interrupted_result);
}

/// `resume()` on a session that was never paused must fail rather than
/// silently succeed — resuming implies there was something to resume from.
#[test]
fn resume_without_pause_is_rejected() {
    let replica_a = ReplicaId::new_random();
    let replica_b = ReplicaId::new_random();
    let org_id = OrganizationId::new_random();

    let mut session = SynchronizationSession::new(replica_a, replica_b, org_id);
    assert!(session.resume().is_err());
}
