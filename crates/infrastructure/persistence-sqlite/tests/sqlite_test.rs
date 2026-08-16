//! Integration test: SQLite repository round-trip against an in-memory
//! SQLite database. Exercises `SqliteRepository` + `SqliteUnitOfWork`
//! with a real `mission_domain::Mission`, the SQLite migration schema,
//! and the full commit/reload cycle. Uses sqlx's `sqlite::SqlitePoolOptions`
//! with an in-memory URL — no file system or Docker dependency.

use mission_domain::test_support::{test_context, test_user_id};
use mission_domain::{Mission, MissionCommand};
use platform_contracts::AggregateRoot;
use query_application::{Repository, UnitOfWork};
use sqlx::sqlite::SqlitePoolOptions;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1) // in-memory DB: single connection to keep state across queries
        .connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory SQLite database");

    // Apply the schema migration inline (no sqlx-cli needed for tests).
    let schema = include_str!("../../../../migrations/sqlite/20260101000000_initial_schema.up.sql");
    // SQLite can't execute multiple statements in one query() call;
    // split on ";" and execute each statement individually, skipping blanks.
    for stmt in schema.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt).execute(&pool).await.unwrap_or_else(|e| {
            panic!("failed to apply SQLite schema statement:\n{stmt}\n\nError: {e}")
        });
    }

    pool
}

fn create_test_mission() -> (Mission, platform_kernel::OrganizationId) {
    let context = test_context();
    let organization_id = context.actor.organization_id;
    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "SQLite Test Mission".to_string(),
            description: Some("Testing SQLite persistence adapter".to_string()),
            owner_id: test_user_id(),
        },
        &context,
    )
    .expect("Mission::create should succeed");
    (Mission::from_created_event(&events[0]), organization_id)
}

#[tokio::test]
async fn sqlite_mission_persists_and_reloads_with_identical_state() {
    let pool = test_pool().await;
    let repo = persistence_sqlite::SqliteRepository::new(pool.clone(), "mission");
    let (mission, organization_id) = create_test_mission();
    let aggregate_state = serde_json::to_value(&mission).expect("Mission must serialize");

    let mut uow = persistence_sqlite::SqliteUnitOfWork::begin(&pool, organization_id)
        .await
        .expect("begin should succeed");

    let commit_result = repo
        .commit(aggregate_state.clone(), &[], &mut uow)
        .await
        .expect("Repository::commit should succeed");

    assert_eq!(commit_result.new_version.0, mission.version().0);
    uow.commit()
        .await
        .expect("UnitOfWork::commit should succeed");

    let loaded = repo
        .load(&platform_kernel::ObjectId(mission.id().0 .0))
        .await
        .expect("load should succeed")
        .expect("aggregate should exist after commit");

    assert_eq!(
        loaded.aggregate, aggregate_state,
        "reloaded state must match committed state"
    );
    assert_eq!(loaded.version.0, mission.version().0);

    // Deserialize back to a real Mission to verify field-level round-trip.
    let reloaded: Mission = serde_json::from_value(loaded.aggregate)
        .expect("reloaded JSON must deserialize into Mission");
    assert_eq!(
        serde_json::to_value(&reloaded).unwrap(),
        serde_json::to_value(&mission).unwrap(),
        "deserialized Mission must be identical to the original"
    );
}

#[tokio::test]
async fn sqlite_concurrent_version_conflict_is_rejected() {
    let pool = test_pool().await;
    let repo = persistence_sqlite::SqliteRepository::new(pool.clone(), "mission");
    let (mission, organization_id) = create_test_mission();
    let initial_state = serde_json::to_value(&mission).unwrap();

    // First write succeeds.
    let mut uow_a = persistence_sqlite::SqliteUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    repo.commit(initial_state.clone(), &[], &mut uow_a)
        .await
        .unwrap();
    uow_a.commit().await.unwrap();

    // Second write with same version must fail.
    let mut uow_b = persistence_sqlite::SqliteUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    repo.commit(initial_state.clone(), &[], &mut uow_b)
        .await
        .unwrap();
    assert!(
        uow_b.commit().await.is_err(),
        "same-version write against existing row must fail (optimistic concurrency)"
    );

    // Advanced version write must succeed.
    let mut advanced = initial_state.clone();
    advanced["version"] = serde_json::json!(1);
    let mut uow_c = persistence_sqlite::SqliteUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    repo.commit(advanced.clone(), &[], &mut uow_c)
        .await
        .unwrap();
    uow_c
        .commit()
        .await
        .expect("advanced-version write must succeed");

    let loaded = repo
        .load(&platform_kernel::ObjectId(mission.id().0 .0))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.version.0, 1);
}

#[tokio::test]
async fn sqlite_outbox_and_idempotency_written_atomically() {
    let pool = test_pool().await;
    let repo = persistence_sqlite::SqliteRepository::new(pool.clone(), "mission");
    let (mission, organization_id) = create_test_mission();
    let initial_state = serde_json::to_value(&mission).unwrap();

    let operation_id = platform_kernel::OperationId::new_random();
    let mut uow = persistence_sqlite::SqliteUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();

    repo.commit(initial_state, &[], &mut uow).await.unwrap();
    uow.register_outbox(query_application::OutboxMessage {
        event_id: platform_kernel::EventId(*uuid::Uuid::new_v4().as_bytes()),
        event_type: "MissionCreated".to_string(),
        aggregate_id: "test".to_string(),
        organization_id,
        payload: b"{}".to_vec(),
        vector_clock: platform_kernel::VectorClock::new(),
        occurred_at: platform_kernel::Timestamp::now(),
    });
    uow.register_idempotency_result(operation_id, serde_json::json!({ "success": true }));
    uow.commit().await.expect("commit should succeed");

    let outbox_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(outbox_count, 1, "outbox row should be present");

    let idem_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM idempotency")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(idem_count, 1, "idempotency row should be present");
}

/// Regression test for ruling V1 (`DECISIONS.md`): a non-empty
/// `VectorClock` previously failed to serialize / was silently masked to
/// `"{}"` on write. Proves the fix holds through `SqliteOutboxStore`'s
/// real `claim_unpublished`, not just at the `VectorClock` unit level.
#[tokio::test]
async fn sqlite_non_empty_vector_clock_round_trips_through_outbox() {
    use worker_application::OutboxStore;

    let pool = test_pool().await;
    let organization_id = platform_kernel::OrganizationId::new_random();

    let replica_a = platform_kernel::ReplicaId::new_random();
    let replica_b = platform_kernel::ReplicaId::new_random();
    let mut vector_clock = platform_kernel::VectorClock::new();
    vector_clock.increment(replica_a);
    vector_clock.increment(replica_a);
    vector_clock.increment(replica_b);

    let mut uow = persistence_sqlite::SqliteUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    uow.register_outbox(query_application::OutboxMessage {
        event_id: platform_kernel::EventId(*uuid::Uuid::new_v4().as_bytes()),
        event_type: "MissionCreated".to_string(),
        aggregate_id: "test".to_string(),
        organization_id,
        payload: b"{}".to_vec(),
        vector_clock: vector_clock.clone(),
        occurred_at: platform_kernel::Timestamp::now(),
    });
    uow.commit().await.expect(
        "commit must succeed for a non-empty vector clock (previously failed before ruling V1)",
    );

    let outbox_store = persistence_sqlite::SqliteOutboxStore::new(pool.clone());
    let claimed = outbox_store
        .claim_unpublished("relay-1", 10)
        .await
        .expect("claim should succeed");

    assert_eq!(claimed.len(), 1);
    assert_eq!(
        claimed[0].message.vector_clock, vector_clock,
        "the vector clock read back must exactly match what was written"
    );
    assert_eq!(claimed[0].message.vector_clock.get(&replica_a), 2);
    assert_eq!(claimed[0].message.vector_clock.get(&replica_b), 1);
}

/// Regression test for ruling V2 (`DECISIONS.md`): `claim_unpublished`'s
/// read-side fallback previously silently defaulted to an empty
/// `VectorClock` for any row whose `vector_clock` TEXT column wasn't
/// valid JSON, or was valid JSON of the wrong shape. Unlike Postgres's
/// `JSONB` column (which enforces JSON syntax at the database level),
/// SQLite's `TEXT` column enforces nothing — so this test can and does
/// write genuinely malformed (non-JSON) text directly, the most extreme
/// version of a "corrupt row."
#[tokio::test]
async fn sqlite_claim_unpublished_returns_an_error_for_a_malformed_vector_clock_column() {
    use worker_application::{OutboxError, OutboxStore};

    let pool = test_pool().await;
    let organization_id = platform_kernel::OrganizationId::new_random();

    let mut uow = persistence_sqlite::SqliteUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    uow.register_outbox(query_application::OutboxMessage {
        event_id: platform_kernel::EventId(*uuid::Uuid::new_v4().as_bytes()),
        event_type: "MissionCreated".to_string(),
        aggregate_id: "test".to_string(),
        organization_id,
        payload: b"{}".to_vec(),
        vector_clock: platform_kernel::VectorClock::new(),
        occurred_at: platform_kernel::Timestamp::now(),
    });
    uow.commit().await.unwrap();

    // Directly corrupt the just-committed row's vector_clock column with
    // text that isn't even valid JSON syntax (SQLite's TEXT column
    // permits this, unlike Postgres's JSONB).
    sqlx::query("UPDATE outbox SET vector_clock = 'not even json {'")
        .execute(&pool)
        .await
        .expect("direct corruption of the test row should succeed");

    let outbox_store = persistence_sqlite::SqliteOutboxStore::new(pool.clone());
    let result = outbox_store.claim_unpublished("relay-1", 10).await;

    match result {
        Err(OutboxError::Serialization(msg)) => {
            assert!(!msg.is_empty(), "error message should be informative");
        }
        other => {
            panic!("expected Err(OutboxError::Serialization(_)) for a corrupt row, got {other:?}")
        }
    }
}
