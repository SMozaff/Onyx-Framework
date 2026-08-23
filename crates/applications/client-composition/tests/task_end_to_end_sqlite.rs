//! End-to-end integration test: `TaskCreationHandler` and
//! `TaskDecisionHandler` against a real, live SQLite database. Mirrors
//! `mission_end_to_end_sqlite.rs`'s structure and purpose (see that
//! file's module doc comment for the significance of this pattern —
//! ruling F1 in `DECISIONS.md`).
//!
//! Unlike the Mission test's `PauseMission` step (which legitimately
//! fails from `Mission`'s initial `Draft` status, so that test only
//! asserts "a real domain decision was reached"), `Task`'s `MarkReady`
//! command succeeds from `Task::create()`'s initial `Draft` status (per
//! Team 1's ruling B: `Draft → Ready` via `MarkReady`) — so this test
//! asserts success deterministically, giving slightly stronger coverage
//! of the full round-trip.

use client_composition::command_registry::CommandRegistry;
use mission_domain::test_support::test_user_id;
use persistence_sqlite::{SqliteRepository, SqliteUnitOfWorkFactory};
use platform_contracts::CommandEnvelope;
use platform_kernel::{
    AuthorityEpoch, AuthorityProof, AuthorityScope, CommandId, CorrelationId, DomainObjectRef,
    LifecycleEpoch, ObjectId, ObjectVersion, OrganizationId, ProofType, SchemaVersion, Timestamp,
    VectorClock,
};
use std::sync::Arc;

#[path = "support/mod.rs"]
mod support;
use support::{test_pool, InMemoryIdempotencyStore};

fn envelope_for(
    command_type: &str,
    target_id: ObjectId,
    organization_id: OrganizationId,
    expected_version: u64,
    payload: serde_json::Value,
) -> CommandEnvelope<serde_json::Value> {
    CommandEnvelope {
        command_id: CommandId::new_random(),
        operation_id: platform_kernel::OperationId::new_random(),
        command_type: command_type.to_string(),
        schema_version: SchemaVersion::new("1.0.0"),
        target: DomainObjectRef {
            id: target_id,
            r#type: "task".to_string(),
            organization_id,
        },
        expected_version: ObjectVersion(expected_version),
        expected_lifecycle_epoch: LifecycleEpoch(0),
        expected_authority_epoch: AuthorityEpoch(0),
        actor: platform_kernel::ActorContext {
            user_id: test_user_id(),
            device_id: ObjectId::new_random(),
            organization_id,
        },
        authority_proof: AuthorityProof {
            proof_type: ProofType::Jwt,
            scope: AuthorityScope {
                organization_id,
                object_type: "task".to_string(),
                object_id: None,
                command_types: vec![command_type.to_string()],
                delegation_depth: 0,
            },
            issued_at: Timestamp::from_nanos(0),
            expires_at: Timestamp::from_nanos(u64::MAX),
            signature: None,
        },
        issued_at: Timestamp::now(),
        vector_clock: VectorClock::new(),
        correlation_id: CorrelationId::new_random(),
        causation_id: None,
        payload,
    }
}

#[tokio::test]
async fn create_task_then_mark_it_ready_through_the_real_command_registry_and_sqlite() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let mission_id = ObjectId::new_random(); // TaskMissionRef = MissionId(ObjectId); no real Mission needed for this test's assertions.

    let repo: Arc<dyn query_application::Repository> =
        Arc::new(SqliteRepository::new(pool.clone(), "task"));
    let unit_factory: Arc<dyn query_application::UnitOfWorkFactory> =
        Arc::new(SqliteUnitOfWorkFactory::new(pool.clone()));
    let idempotency_store: Arc<dyn query_application::IdempotencyStore> =
        Arc::new(InMemoryIdempotencyStore::new());

    let mut registry = CommandRegistry::new();
    registry
        .register_creation(
            "CreateTask",
            client_composition::handlers::TaskCreationHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
            ),
        )
        .register_decision(
            "MarkReady",
            client_composition::handlers::TaskDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
                Arc::new(support::AllowAllOwnerAuthority),
            ),
        );

    // 1. Create the task.
    let create_envelope = envelope_for(
        "CreateTask",
        ObjectId::new_random(), // ignored by CreationHandler; no existing aggregate yet
        organization_id,
        0,
        serde_json::json!({
            "CreateTask": {
                "mission_id": mission_id,
                "title": "Integration Test Task",
                "description": "Proves handle_command works for Task against real SQLite",
                "owner_id": test_user_id(),
            }
        }),
    );
    let create_result = registry
        .dispatch(create_envelope)
        .await
        .expect("CreateTask should succeed against a real SQLite database");
    let task_id: ObjectId = serde_json::from_value(create_result["task_id"].clone())
        .expect("task_id must be a valid ObjectId");

    // 2. Confirm it's genuinely persisted.
    let loaded = repo
        .load(&task_id)
        .await
        .expect("load should succeed")
        .expect("task must exist after CreateTask commit");
    assert_eq!(loaded.version.0, 0);

    // 3. Mark it ready (Draft -> Ready, per Team 1 ruling B — succeeds
    // deterministically from a freshly created Task, unlike Mission's
    // PauseMission which legitimately rejects from Draft).
    let ready_envelope = envelope_for(
        "MarkReady",
        task_id,
        organization_id,
        0,
        serde_json::json!({"MarkReady": {"reason": "integration test"}}),
    );
    let ready_result = registry
        .dispatch(ready_envelope)
        .await
        .expect("MarkReady must succeed from a freshly created Task's Draft status");
    assert_eq!(ready_result["success"], serde_json::json!(true));
    assert_eq!(ready_result["new_version"], serde_json::json!(1));

    // 4. Confirm the status transition genuinely persisted.
    let reloaded = repo.load(&task_id).await.unwrap().unwrap();
    assert_eq!(
        reloaded.version.0, 1,
        "version must have advanced after MarkReady committed"
    );
    assert_eq!(
        reloaded.aggregate["status"],
        serde_json::json!("Ready"),
        "persisted status must reflect the Draft -> Ready transition"
    );
}

/// Regression test for ruling H3 (`DECISIONS.md`): `handle_command`
/// previously serialized the aggregate's *pre-decide* state (never
/// calling `apply()`), so the persisted `version` field never advanced —
/// every SECOND successful decision command against the same aggregate
/// failed both adapters' `WHERE version < excluded.version` optimistic-
/// concurrency guard, since the write's version was never actually
/// higher than what was already stored. This is Team 2's own previously
/// self-documented "H1" gap (a different DECISIONS.md, different label —
/// see the H2/H3 entries in the master `DECISIONS.md` for the exact
/// cross-reference), now fixed.
///
/// This test chains THREE successive decision commands against the same
/// `Task` (`MarkReady`, then two `AssignOwner`s — a self-loop-permitted
/// command per Team 1's ruling, valid repeatedly from `Ready`), each
/// with `expected_version` incrementing by exactly one, and asserts the
/// stored version and event count advance correctly through every step —
/// the strongest direct evidence the persisted version now genuinely
/// tracks reality rather than being permanently stuck.
#[tokio::test]
async fn version_advances_correctly_across_three_successive_decision_commands() {
    let pool = test_pool().await;
    let organization_id = OrganizationId::new_random();
    let mission_id = ObjectId::new_random();

    let repo: Arc<dyn query_application::Repository> =
        Arc::new(SqliteRepository::new(pool.clone(), "task"));
    let unit_factory: Arc<dyn query_application::UnitOfWorkFactory> =
        Arc::new(SqliteUnitOfWorkFactory::new(pool.clone()));
    let idempotency_store: Arc<dyn query_application::IdempotencyStore> =
        Arc::new(InMemoryIdempotencyStore::new());

    let mut registry = CommandRegistry::new();
    registry
        .register_creation(
            "CreateTask",
            client_composition::handlers::TaskCreationHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
            ),
        )
        .register_decision(
            "MarkReady",
            client_composition::handlers::TaskDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
                Arc::new(support::AllowAllOwnerAuthority),
            ),
        )
        .register_decision(
            "AssignOwner",
            client_composition::handlers::TaskDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
                Arc::new(support::AllowAllOwnerAuthority),
            ),
        );

    let create_result = registry
        .dispatch(envelope_for(
            "CreateTask",
            ObjectId::new_random(),
            organization_id,
            0,
            serde_json::json!({
                "CreateTask": {
                    "mission_id": mission_id,
                    "title": "Version Advancement Regression Task",
                    "description": null,
                    "owner_id": test_user_id(),
                }
            }),
        ))
        .await
        .unwrap();
    let task_id: ObjectId = serde_json::from_value(create_result["task_id"].clone()).unwrap();

    let commands = [
        (
            "MarkReady",
            serde_json::json!({"MarkReady": {"reason": "step 1"}}),
        ),
        (
            "AssignOwner",
            serde_json::json!({"AssignOwner": {"owner_id": test_user_id(), "reason": "step 2"}}),
        ),
        (
            "AssignOwner",
            serde_json::json!({"AssignOwner": {"owner_id": test_user_id(), "reason": "step 3"}}),
        ),
    ];

    // Tracks the real lifecycle_epoch across the sequence: MarkReady
    // (Draft -> Ready) is a status transition and genuinely advances it
    // (Task::apply's TaskMarkedReady arm calls `lifecycle_epoch.advance()`);
    // AssignOwner does not change status and advances authority_epoch
    // instead, leaving lifecycle_epoch unchanged. This is real, correct
    // domain behavior that H3's fix now actually surfaces — asserting a
    // fixed LifecycleEpoch(0) for every command (an earlier draft of this
    // test did) was a bug in the test, not in the fix.
    let mut expected_lifecycle_epoch = 0u64;
    // The comment above already states AssignOwner advances authority_epoch,
    // but nothing tracked it: the envelope left expected_authority_epoch at
    // its default 0, so the second AssignOwner was rejected with
    // AuthorityEpochConflict { expected: 0, actual: 1 }. The domain was right
    // — owner_id is authority-controlled for Work, so reassignment advances
    // the epoch — and the test simply never asserted the half of the
    // behaviour it had described.
    let mut expected_authority_epoch = 0u64;

    for (expected_version, (command_type, payload)) in commands.into_iter().enumerate() {
        let mut envelope = envelope_for(
            command_type,
            task_id,
            organization_id,
            expected_version as u64,
            payload,
        );
        envelope.expected_lifecycle_epoch =
            platform_kernel::LifecycleEpoch(expected_lifecycle_epoch);
        envelope.expected_authority_epoch =
            platform_kernel::AuthorityEpoch(expected_authority_epoch);

        let result = registry.dispatch(envelope).await.unwrap_or_else(|e| {
            panic!("command {expected_version} ({command_type}) should succeed: {e:?}")
        });

        if command_type == "MarkReady" {
            expected_lifecycle_epoch += 1;
        }
        if command_type == "AssignOwner" {
            expected_authority_epoch += 1;
        }

        let expected_new_version = expected_version as u64 + 1;
        assert_eq!(
            result["new_version"],
            serde_json::json!(expected_new_version),
            "command {expected_version} ({command_type}) should report new_version {expected_new_version}"
        );

        let loaded = repo.load(&task_id).await.unwrap().unwrap();
        assert_eq!(
            loaded.version.0, expected_new_version,
            "stored version after command {expected_version} ({command_type}) should be {expected_new_version}"
        );
    }
}
