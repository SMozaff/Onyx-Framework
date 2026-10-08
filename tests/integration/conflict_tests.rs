//! Authority Conflict Test.
//! Source: Team Prompt 3 §6.2, rebuilt against the real, generic
//! `DomainEventEnvelope<serde_json::Value>` and `ActorContext`/
//! `AuditMetadata` (Architectural Ruling Q1).
//!
//! Fixes carried over from the original defect report:
//! - `SynchronizationSession::new(replica_a, replica_b)` (2 args) →
//!   `new(local, remote, organization_id)` (3 args): `ConflictRecord`
//!   requires an `organization_id`.
//! - `session.exchange_operations(...)` / `session.reconcile()` called on a
//!   `mut` binding (both take `&mut self`).
//! - `DomainEventEnvelope { /* ActivateMission */ }` — struct-update
//!   shorthand with no fields is not valid Rust; this test constructs two
//!   full envelopes with concurrent vector clocks and a
//!   `payload.field = "status"` so `session::extract_field_path` resolves
//!   to `"status"`, matching the frozen assertion.
//! - The session must be given field metadata marking `"status"` as
//!   authority-controlled — §6.2 never shows this step but without it
//!   `reconcile()` has no way to know `"status"` isn't CRDT-mergeable.

use platform_contracts::{AuditMetadata, DataClassification, DomainEventEnvelope};
use platform_kernel::{
    ActorContext, AuthorityEpoch, CorrelationId, DomainObjectRef, EventId, LifecycleEpoch,
    ObjectId, ObjectVersion, OperationId, OrganizationId, ReplicaId, SchemaVersion, Timestamp,
    VectorClock,
};
use synchronization_domain::{ConflictStatus, FieldMetadata, FieldShape, SynchronizationSession};

fn authority_envelope(
    aggregate_id: ObjectId,
    org_id: OrganizationId,
    replica: ReplicaId,
    event_type: &str,
) -> DomainEventEnvelope<serde_json::Value> {
    let mut clock = VectorClock::new();
    clock.increment(replica); // count = 1 on this replica only
    DomainEventEnvelope {
        event_id: EventId::new_random(),
        event_type: event_type.to_string(),
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
        payload: serde_json::json!({ "field": "status" }),
    }
}

#[test]
fn concurrent_authority_writes_create_conflict() {
    let replica_a = ReplicaId::new_random();
    let replica_b = ReplicaId::new_random();
    let org_id = OrganizationId::new_random();
    let aggregate_id = ObjectId::new_random();

    let mut session = SynchronizationSession::new(replica_a, replica_b, org_id)
        .with_field_metadata(vec![FieldMetadata {
            name: "status".to_string(),
            shape: FieldShape::AuthorityControlled,
            is_authority_controlled: true,
            is_crdt_eligible: false,
        }]);

    // Two concurrent operations on the authority-controlled "status" field.
    // Different replicas, each with count=1 on only its own replica —
    // neither dominates the other, so VectorClock::compare returns
    // CONCURRENT.
    let op_a = authority_envelope(aggregate_id, org_id, replica_a, "ActivateMission");
    let op_b = authority_envelope(aggregate_id, org_id, replica_b, "PauseMission");

    session.exchange_operations(vec![op_a]).unwrap();
    session.exchange_operations(vec![op_b]).unwrap();
    let conflicts = session.reconcile().unwrap();

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].field_path, "status");
    assert_eq!(conflicts[0].status, ConflictStatus::Open);
}

/// §8 acceptance criteria name a `conflict_pending_rejects_commands` test
/// that appears nowhere in §6. Implemented here.
#[test]
fn conflict_pending_rejects_commands() {
    use platform_contracts::CommandEnvelope;
    use platform_contracts_ext::{AggregateRoot, ConflictPendingMarker};
    use synchronization_domain::{check_conflict_pending, GuardError};

    struct FakeAggregate {
        pending: Option<ConflictPendingMarker>,
    }
    impl AggregateRoot for FakeAggregate {
        type Id = ObjectId;
        type Command = ();
        type Event = ();
        type Error = std::convert::Infallible;

        fn id(&self) -> &Self::Id {
            unimplemented!("not exercised by this test")
        }
        fn version(&self) -> ObjectVersion {
            ObjectVersion::INITIAL
        }
        fn lifecycle_epoch(&self) -> LifecycleEpoch {
            LifecycleEpoch::INITIAL
        }
        fn authority_epoch(&self) -> AuthorityEpoch {
            AuthorityEpoch::INITIAL
        }
        fn decide(
            &self,
            _command: Self::Command,
            _context: &platform_contracts_ext::DecisionContext,
        ) -> Result<Vec<Self::Event>, Self::Error> {
            unimplemented!("not exercised by this test")
        }
        fn apply(&mut self, _event: &Self::Event) {}

        fn conflict_pending(&self) -> Option<ConflictPendingMarker> {
            self.pending
        }
    }

    let conflict_id = platform_kernel::ConflictId([7u8; 16]);
    let aggregate = FakeAggregate {
        pending: Some(ConflictPendingMarker {
            conflict_id,
            since: Timestamp::now(),
        }),
    };

    let org_id = OrganizationId::new_random();
    let mutating_command = CommandEnvelope {
        command_id: platform_kernel::CommandId::new_random(),
        operation_id: OperationId::new_random(),
        command_type: "mutate.UpdateStatus".to_string(),
        schema_version: SchemaVersion::new("1.0"),
        target: DomainObjectRef {
            id: ObjectId::new_random(),
            r#type: "mission".to_string(),
            organization_id: org_id,
        },
        expected_version: ObjectVersion::INITIAL,
        expected_lifecycle_epoch: LifecycleEpoch::INITIAL,
        expected_authority_epoch: AuthorityEpoch::INITIAL,
        actor: ActorContext {
            user_id: ObjectId::new_random(),
            device_id: ObjectId::new_random(),
            organization_id: org_id,
        },
        authority_proof: platform_kernel::AuthorityProof {
            proof_type: platform_kernel::ProofType::Jwt,
            scope: platform_kernel::AuthorityScope {
                organization_id: org_id,
                object_type: "mission".to_string(),
                object_id: None,
                command_types: vec!["UpdateStatus".to_string()],
                delegation_depth: 0,
            },
            issued_at: Timestamp::now(),
            expires_at: Timestamp::now(),
            signature: None,
        },
        issued_at: Timestamp::now(),
        vector_clock: VectorClock::new(),
        correlation_id: CorrelationId::new_random(),
        causation_id: None,
        payload: (),
    };

    let result = check_conflict_pending(&aggregate, &mutating_command);
    assert_eq!(result, Err(GuardError::ConflictPending { conflict_id }));

    // Query commands must still be allowed through.
    let mut query_command = mutating_command.clone();
    query_command.command_type = "query.GetStatus".to_string();
    assert!(check_conflict_pending(&aggregate, &query_command).is_ok());

    // No conflict pending -> everything allowed.
    let clear_aggregate = FakeAggregate { pending: None };
    assert!(check_conflict_pending(&clear_aggregate, &mutating_command).is_ok());
}
