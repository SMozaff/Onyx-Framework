//! Integration test: optimistic concurrency control on the `aggregates`
//! table — verifying that a stale-version write is rejected, per Handover
//! §8's versioning contract. One of the 4 required integration test
//! suites per Team Prompt 2 §2 File Manifest ("concurrency" suite).

use mission_domain::test_support::{test_context, test_user_id};
use mission_domain::{Mission, MissionCommand};
use platform_contracts::AggregateRoot;
use query_application::{Repository, UnitOfWork};
use sqlx::postgres::PgPoolOptions;
use std::sync::OnceLock;
use tokio::sync::Mutex;

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

fn create_mission() -> (Mission, platform_kernel::OrganizationId) {
    let context = test_context();
    let organization_id = context.actor.organization_id;
    let owner_id = test_user_id();
    let events = Mission::create(
        MissionCommand::CreateMission {
            name: "Concurrency Test Mission".to_string(),
            description: None,
            owner_id,
        },
        &context,
    )
    .expect("Mission::create should succeed");
    (Mission::from_created_event(&events[0]), organization_id)
}

#[tokio::test]
async fn second_writer_with_stale_version_is_rejected() {
    let (pool, _guard) = test_pool().await;
    let repo = persistence_postgres::PostgresRepository::new(pool.clone(), "mission");

    let (mission, organization_id) = create_mission();
    let initial_state = serde_json::to_value(&mission).unwrap();

    // Writer A: commits the initial state at version 0 (INITIAL).
    let mut uow_a = persistence_postgres::PostgresUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    repo.commit(initial_state.clone(), &[], &mut uow_a)
        .await
        .unwrap();
    uow_a.commit().await.unwrap();

    // Writer B "loads" the same version-0 state (simulating two
    // concurrent command handlers that both read before either writes)
    // and tries to commit a change WITHOUT bumping the version — this
    // must be rejected, since a same-version write against an
    // already-persisted row is stale by definition.
    let mut uow_b = persistence_postgres::PostgresUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    let stale_write_result = repo.commit(initial_state.clone(), &[], &mut uow_b).await;

    // Repository::commit stages the upsert (doesn't execute it
    // immediately — the actual write happens inside UnitOfWork::commit,
    // per the atomicity requirement), so the conflict surfaces at
    // uow_b.commit() time, not at repo.commit() time.
    assert!(
        stale_write_result.is_ok(),
        "staging the write itself should succeed (no I/O yet)"
    );

    let commit_result = uow_b.commit().await;
    assert!(
        commit_result.is_err(),
        "committing a same-version (stale) write against an already-existing row must fail"
    );
}

#[tokio::test]
async fn writer_with_advanced_version_succeeds() {
    let (pool, _guard) = test_pool().await;
    let repo = persistence_postgres::PostgresRepository::new(pool.clone(), "mission");

    let (mission, organization_id) = create_mission();
    let initial_state = serde_json::to_value(&mission).unwrap();

    let mut uow_a = persistence_postgres::PostgresUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    repo.commit(initial_state.clone(), &[], &mut uow_a)
        .await
        .unwrap();
    uow_a.commit().await.unwrap();

    // A legitimate second write bumps the version field before
    // committing, exactly as a real command handler would after calling
    // AggregateRoot::apply() on a new event.
    let mut advanced_state = initial_state.clone();
    advanced_state["version"] = serde_json::json!(1);

    let mut uow_c = persistence_postgres::PostgresUnitOfWork::begin(&pool, organization_id)
        .await
        .unwrap();
    repo.commit(advanced_state.clone(), &[], &mut uow_c)
        .await
        .unwrap();
    let result = uow_c.commit().await;

    assert!(
        result.is_ok(),
        "a write with an advanced version must succeed: {:?}",
        result
    );

    let loaded = repo
        .load(&platform_kernel::ObjectId(extract_mission_id(&mission)))
        .await
        .unwrap()
        .expect("aggregate should exist");
    assert_eq!(
        loaded.version.0, 1,
        "stored version should reflect the successful advanced write"
    );
}

fn extract_mission_id(mission: &Mission) -> [u8; 16] {
    mission.id().0 .0
}
