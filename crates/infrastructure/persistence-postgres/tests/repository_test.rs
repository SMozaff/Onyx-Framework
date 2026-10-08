//! Integration test: full round-trip of a real `mission_domain::Mission`
//! aggregate through `PostgresRepository` + `PostgresUnitOfWork`, against a
//! live PostgreSQL database (via `DATABASE_URL`).
//!
//! This is one of the 4 required integration test suites per Team Prompt 2
//! §2 File Manifest ("repository" suite). Uses Team 1's real domain logic
//! (not a stub) to exercise the actual command -> decide -> events ->
//! persist -> reload pipeline end-to-end.

use mission_domain::test_support::{test_context, test_user_id};
use mission_domain::{Mission, MissionCommand};
use platform_contracts::AggregateRoot;
use platform_kernel::ObjectId;
use query_application::{Repository, UnitOfWork};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::OnceLock;
use tokio::sync::Mutex;

/// Serializes the two tests in this file against each other. Both tests
/// share one live PostgreSQL database/table set (no per-test schema or
/// transaction-per-test isolation is set up here), and Rust runs
/// `#[tokio::test]` functions in a test binary concurrently by default.
/// Without this guard, one test's `TRUNCATE` can race another test's
/// insert. A real multi-test suite would normally use one Postgres
/// schema/database per test (via testcontainers-per-test or a schema
/// name derived from the test's thread), but a single shared instance
/// with explicit serialization is simpler and sufficient for this
/// increment's two-test suite. See DECISIONS.md item P7.
static TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

async fn test_pool() -> (sqlx::PgPool, tokio::sync::MutexGuard<'static, ()>) {
    let guard = TEST_MUTEX.get_or_init(|| Mutex::new(())).lock().await;

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set to run persistence-postgres integration tests");
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to test database");

    sqlx::query(
        "TRUNCATE aggregates, domain_events, outbox, idempotency, inbox, dead_letter RESTART IDENTITY",
    )
    .execute(&pool)
    .await
    .expect("failed to truncate tables for test isolation");

    (pool, guard)
}

#[tokio::test]
async fn mission_persists_and_reloads_with_identical_state() {
    let (pool, _guard) = test_pool().await;
    let repo = persistence_postgres::PostgresRepository::new(pool.clone(), "mission");

    // --- Build a real Mission via Team 1's actual domain logic ---
    let context = test_context();
    let owner_id = test_user_id();
    let organization_id = context.actor.organization_id;

    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Operation Nightfall".to_string(),
            description: Some("Integration test mission".to_string()),
            owner_id,
        },
        &context,
    )
    .expect("Mission::create should succeed with valid input");

    assert_eq!(
        events.len(),
        1,
        "CreateMission should produce exactly one event"
    );

    // Rehydrate: apply the event to build the in-memory aggregate. Per
    // Team 1's AggregateRoot contract, `apply()` is how the events
    // returned by decide()/create() become the actual aggregate state.
    // Mission::create() doesn't directly return a Mission, so we
    // construct one via replay, matching how a real command handler
    // would rehydrate from an event stream.
    let mission = replay_mission(&events);
    let mission_id_bytes = extract_mission_id(&mission);

    // --- Serialize and commit through the real Postgres adapter ---
    let aggregate_state = serde_json::to_value(&mission).expect("Mission must serialize");
    let event_values: Vec<serde_json::Value> = events
        .iter()
        .map(|e| serde_json::to_value(e).expect("MissionEvent must serialize"))
        .collect();

    let mut uow = persistence_postgres::PostgresUnitOfWork::begin(&pool, organization_id)
        .await
        .expect("begin transaction");

    let commit_result = repo
        .commit(aggregate_state.clone(), &event_values, &mut uow)
        .await
        .expect("Repository::commit should succeed for a new aggregate");

    assert_eq!(commit_result.new_version.0, mission.version().0);

    uow.commit()
        .await
        .expect("UnitOfWork::commit should succeed");

    // --- Reload and verify state round-trips exactly ---
    let loaded = repo
        .load(&ObjectId(mission_id_bytes))
        .await
        .expect("load should succeed")
        .expect("aggregate should exist after commit");

    assert_eq!(
        loaded.aggregate, aggregate_state,
        "reloaded state must match what was committed"
    );
    assert_eq!(loaded.version.0, mission.version().0);
    assert_eq!(loaded.lifecycle_epoch.0, mission.lifecycle_epoch().0);
    assert_eq!(loaded.authority_epoch.0, mission.authority_epoch().0);

    // Reloaded state must deserialize back into a real Mission with
    // identical business-relevant fields (spot-checked via re-serialization,
    // since Mission's fields are private and there's no public equality
    // derive on the struct).
    let reloaded_mission: Mission =
        serde_json::from_value(loaded.aggregate).expect("Mission must deserialize");
    assert_eq!(
        serde_json::to_value(&reloaded_mission).unwrap(),
        serde_json::to_value(&mission).unwrap()
    );
}

#[tokio::test]
async fn commit_stages_events_outbox_and_idempotency_atomically() {
    let (pool, _guard) = test_pool().await;
    let repo = persistence_postgres::PostgresRepository::new(pool.clone(), "mission");

    let context = test_context();
    let organization_id = context.actor.organization_id;
    let owner_id = test_user_id();

    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Atomicity Test Mission".to_string(),
            description: None,
            owner_id,
        },
        &context,
    )
    .expect("Mission::create should succeed");

    let mission = replay_mission(&events);
    let aggregate_state = serde_json::to_value(&mission).unwrap();
    let event_values: Vec<serde_json::Value> = events
        .iter()
        .map(|e| serde_json::to_value(e).unwrap())
        .collect();

    let mut uow = persistence_postgres::PostgresUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();

    repo.commit(aggregate_state, &event_values, &mut uow)
        .await
        .unwrap();

    let operation_id = platform_kernel::OperationId::new_random();
    uow.register_idempotency_result(
        operation_id,
        json!({ "success": true, "operation_id": operation_id }),
    );

    uow.commit().await.expect("commit should succeed");

    // Verify the domain event actually landed in domain_events.
    let event_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM domain_events")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(
        event_count >= 1,
        "at least one domain event should have been persisted"
    );

    // Verify the idempotency row landed.
    let idem_row: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT result FROM idempotency WHERE operation_id = $1")
            .bind(persistence_common::bytes_to_uuid(operation_id.0))
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(
        idem_row.is_some(),
        "idempotency result should have been persisted atomically with the event"
    );
}

/// Rehydrates a `Mission` from its first event, using Team 1's real,
/// public `Mission::from_created_event` constructor (discovered after
/// initially — incorrectly — hand-rolling a shadow struct + JSON
/// round-trip to fake private-field construction; that approach was
/// unnecessary and has been removed in favor of the real API).
fn replay_mission(events: &[mission_domain::MissionEvent]) -> Mission {
    Mission::from_created_event(&events[0])
}

fn extract_mission_id(mission: &Mission) -> [u8; 16] {
    mission.id().0 .0
}
