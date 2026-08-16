//! End-to-end integration test: `MissionCreationHandler` and
//! `MissionDecisionHandler` against a real, live SQLite database.
//!
//! # Significance (see `DECISIONS.md`, ruling F1)
//! This is the first test in the entire ONYX workspace to exercise
//! `api_server::handle_command()` against a real persistence adapter.
//! Every prior test of the command pipeline's *logic* either bypassed
//! `handle_command` and drove `Repository`/`UnitOfWork` directly, or used
//! `sync-test-utils`'s in-memory mocks — because no production
//! `UnitOfWorkFactory` existed anywhere in Increments 1–4 before ruling
//! F1 added `SqliteUnitOfWorkFactory`/`PostgresUnitOfWorkFactory`.

use client_composition::command_registry::{CommandDispatchError, CommandRegistry};
use client_composition::handlers::{MissionCreationHandler, MissionDecisionHandler};
use mission_domain::test_support::{test_context, test_user_id};
use persistence_sqlite::{SqliteRepository, SqliteUnitOfWorkFactory};
use platform_contracts::CommandEnvelope;
use platform_kernel::{
    AuthorityEpoch, AuthorityProof, AuthorityScope, CommandId, CorrelationId, DomainObjectRef,
    LifecycleEpoch, ObjectId, ObjectVersion, ProofType, SchemaVersion, Timestamp, VectorClock,
};
use std::sync::Arc;

#[path = "support/mod.rs"]
mod support;
use support::{test_pool, InMemoryIdempotencyStore};

fn envelope_for(
    command_type: &str,
    target_id: ObjectId,
    organization_id: platform_kernel::OrganizationId,
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
            r#type: "mission".to_string(),
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
                object_type: "mission".to_string(),
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
async fn create_mission_then_pause_it_through_the_real_command_registry_and_sqlite() {
    let pool = test_pool().await;
    let organization_id = test_context().actor.organization_id;

    let repo: Arc<dyn query_application::Repository> =
        Arc::new(SqliteRepository::new(pool.clone(), "mission"));
    let unit_factory: Arc<dyn query_application::UnitOfWorkFactory> =
        Arc::new(SqliteUnitOfWorkFactory::new(pool.clone()));
    let idempotency_store: Arc<dyn query_application::IdempotencyStore> =
        Arc::new(InMemoryIdempotencyStore::new());

    let mut registry = CommandRegistry::new();
    registry
        .register_creation(
            "CreateMission",
            MissionCreationHandler::new(Arc::clone(&repo), Arc::clone(&unit_factory)),
        )
        .register_decision(
            "PauseMission",
            MissionDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
            ),
        );

    // 1. Create the mission.
    let create_envelope = envelope_for(
        "CreateMission",
        ObjectId::new_random(), // ignored by CreationHandler; no existing aggregate yet
        organization_id,
        0,
        serde_json::json!({
            "CreateMission": {
                "name": "Integration Test Mission",
                "description": "Proves handle_command works against real SQLite",
                "owner_id": test_user_id(),
            }
        }),
    );
    let create_result = registry
        .dispatch(create_envelope)
        .await
        .expect("CreateMission should succeed against a real SQLite database");
    let mission_id_value = create_result["mission_id"].clone();
    let mission_id: ObjectId =
        serde_json::from_value(mission_id_value).expect("mission_id must be a valid ObjectId");

    // 2. Load it back directly via the repository, to confirm it's
    // genuinely persisted (not merely returned by the handler in memory).
    let loaded = repo
        .load(&mission_id)
        .await
        .expect("load should succeed")
        .expect("mission must exist after CreateMission commit");
    assert_eq!(
        loaded.version.0, 0,
        "a freshly created aggregate has version 0"
    );

    // 3. Pause it through the DecisionHandler path (wraps the real,
    // previously-never-database-tested api_server::handle_command).
    let pause_envelope = envelope_for(
        "PauseMission",
        mission_id,
        organization_id,
        0, // matches the version just loaded
        serde_json::json!({"PauseMission": {"reason": "integration test"}}),
    );
    let pause_result = registry.dispatch(pause_envelope).await;

    // This may legitimately fail if PauseMission isn't valid from
    // Mission's initial (Draft) status per Increment 1's ruled state
    // machine — the point of this assertion is that handle_command
    // executed against the real database and returned a real domain
    // decision, not a connection/plumbing failure.
    match pause_result {
        Ok(_) => {}
        Err(CommandDispatchError::Decision(api_server::CommandError::Domain(_))) => {
            // A legitimate domain rejection (e.g. invalid transition from
            // Draft) is an acceptable outcome for this test — it proves
            // handle_command reached real decide() logic against real
            // persisted state, which is what F1 exists to make possible.
        }
        Err(other) => panic!(
            "expected a real domain decision or success, got infrastructure error: {other:?}"
        ),
    }
}

/// Regression test for ruling H2 (`DECISIONS.md`): `handle_command`
/// previously opened a `UnitOfWork` transaction *before* calling
/// `repo.load()`, deadlocking under a single-connection pool (the
/// standard, near-mandatory configuration for in-memory SQLite) — the
/// open transaction holds the pool's only connection, so `repo.load()`
/// can never acquire one. Confirmed via an isolated, minimal reproduction
/// before the fix (see `DECISIONS.md`). This test proves the fix holds
/// specifically under that failure condition: a real, `max_connections(1)`
/// pool, exercising the exact `DecisionHandler` path (load-then-decide)
/// that deadlocked before H2. A short, explicit timeout is used (rather
/// than relying on an implicit test-runner timeout) so a regression fails
/// fast and unambiguously, distinguishing "deadlocked" from "genuinely
/// slow."
#[tokio::test]
async fn decision_handler_does_not_deadlock_a_single_connection_pool() {
    let pool = test_pool().await;
    let organization_id = test_context().actor.organization_id;

    let repo: Arc<dyn query_application::Repository> =
        Arc::new(SqliteRepository::new(pool.clone(), "mission"));
    let unit_factory: Arc<dyn query_application::UnitOfWorkFactory> =
        Arc::new(SqliteUnitOfWorkFactory::new(pool.clone()));
    let idempotency_store: Arc<dyn query_application::IdempotencyStore> =
        Arc::new(InMemoryIdempotencyStore::new());

    let mut registry = CommandRegistry::new();
    registry
        .register_creation(
            "CreateMission",
            MissionCreationHandler::new(Arc::clone(&repo), Arc::clone(&unit_factory)),
        )
        .register_decision(
            "PauseMission",
            MissionDecisionHandler::new(
                Arc::clone(&repo),
                Arc::clone(&unit_factory),
                Arc::clone(&idempotency_store),
            ),
        );

    let create_envelope = envelope_for(
        "CreateMission",
        ObjectId::new_random(),
        organization_id,
        0,
        serde_json::json!({
            "CreateMission": {
                "name": "Deadlock Regression Mission",
                "description": null,
                "owner_id": test_user_id(),
            }
        }),
    );
    let create_result = registry.dispatch(create_envelope).await.unwrap();
    let mission_id: ObjectId = serde_json::from_value(create_result["mission_id"].clone()).unwrap();

    let pause_envelope = envelope_for(
        "PauseMission",
        mission_id,
        organization_id,
        0,
        serde_json::json!({"PauseMission": {"reason": "deadlock regression test"}}),
    );

    // Before ruling H2, this call would hang until the connection pool's
    // internal acquire timeout fired (30+ seconds); a 5-second bound is
    // generous for a genuinely non-deadlocked in-memory SQLite call while
    // still failing this test fast and clearly if the deadlock regresses.
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        registry.dispatch(pause_envelope),
    )
    .await;

    assert!(
        result.is_ok(),
        "DecisionHandler timed out — the handle_command transaction-ordering deadlock (ruling H2) may have regressed"
    );
}
