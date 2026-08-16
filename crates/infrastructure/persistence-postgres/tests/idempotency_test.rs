//! Integration test: idempotency behavior — `InboxStore` (consumer-side
//! dedup) and the `idempotency` table (command-side dedup, written via
//! `UnitOfWork::register_idempotency_result`) — against a live PostgreSQL
//! database. One of the 4 required integration test suites per Team
//! Prompt 2 §2 File Manifest ("idempotency" suite).

use persistence_postgres::{PostgresInboxStore, PostgresUnitOfWork};
use platform_kernel::{EventId, ObjectId, OperationId, Timestamp};
use query_application::UnitOfWork;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use worker_application::{InboxError, InboxStore};

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
async fn command_idempotency_returns_cached_result_for_duplicate_operation_id() {
    let (pool, _guard) = test_pool().await;
    let org = ObjectId(*uuid::Uuid::new_v4().as_bytes());

    let operation_id = OperationId::new_random();
    let cached_result = json!({
        "success": true,
        "operation_id": operation_id,
        "note": "first execution result"
    });

    let mut uow = PostgresUnitOfWork::begin(&pool, org).await.unwrap();
    uow.register_idempotency_result(operation_id, cached_result.clone());
    uow.commit()
        .await
        .expect("commit should persist idempotency row");

    // Per Handover §16 delivery_guarantees.commands: a duplicate
    // OperationId must return the SAME cached result, not re-execute.
    // We verify this at the storage layer directly (a full
    // IdempotencyStore Postgres impl would query this same table; the
    // command handler pipeline itself is out of scope for Increment 2 —
    // see Team Prompt 2 §1 Mission).
    let stored: serde_json::Value =
        sqlx::query_scalar("SELECT result FROM idempotency WHERE operation_id = $1")
            .bind(persistence_common::bytes_to_uuid(operation_id.0))
            .fetch_one(&pool)
            .await
            .expect("idempotency row must exist");

    assert_eq!(
        stored, cached_result,
        "cached result must match exactly what was first recorded"
    );

    // A second attempt to register the SAME operation_id (simulating a
    // duplicate command submission racing in) must not overwrite the
    // original result — per the Postgres adapter's ON CONFLICT DO
    // NOTHING semantics (see unit_of_work.rs insert_idempotency).
    let mut uow2 = PostgresUnitOfWork::begin(&pool, org).await.unwrap();
    uow2.register_idempotency_result(
        operation_id,
        json!({ "success": true, "operation_id": operation_id, "note": "should be ignored" }),
    );
    uow2.commit()
        .await
        .expect("second commit should succeed (ON CONFLICT DO NOTHING, not an error)");

    let stored_after: serde_json::Value =
        sqlx::query_scalar("SELECT result FROM idempotency WHERE operation_id = $1")
            .bind(persistence_common::bytes_to_uuid(operation_id.0))
            .fetch_one(&pool)
            .await
            .unwrap();

    assert_eq!(
        stored_after, cached_result,
        "first-recorded result must be preserved; duplicate registration must not overwrite it"
    );
}

#[tokio::test]
async fn inbox_store_deduplicates_event_delivery_per_consumer() {
    let (pool, _guard) = test_pool().await;
    let inbox = PostgresInboxStore::new(pool.clone());

    let consumer_id = "projector.mission";
    let event_id = EventId(*uuid::Uuid::new_v4().as_bytes());

    let not_yet = inbox.is_processed(consumer_id, &event_id).await.unwrap();
    assert!(
        !not_yet,
        "event should not be marked processed before first delivery"
    );

    inbox
        .mark_processed(consumer_id, &event_id, Timestamp::now())
        .await
        .expect("first mark_processed should succeed");

    let now_processed = inbox.is_processed(consumer_id, &event_id).await.unwrap();
    assert!(
        now_processed,
        "event should be marked processed after first delivery"
    );

    // Per Architectural Ruling ("InboxStore mark_processed Behavior" —
    // DECISIONS.md I1): a duplicate delivery must return
    // Err(AlreadyProcessed), not silently succeed.
    let duplicate_result = inbox
        .mark_processed(consumer_id, &event_id, Timestamp::now())
        .await;
    assert!(
        matches!(duplicate_result, Err(InboxError::AlreadyProcessed)),
        "duplicate mark_processed must return AlreadyProcessed, got: {:?}",
        duplicate_result
    );

    // A different consumer processing the SAME event is a distinct
    // (consumer_id, event_id) pair and must succeed independently —
    // inbox dedup is per-consumer, not global.
    let other_consumer_result = inbox
        .mark_processed("projector.task", &event_id, Timestamp::now())
        .await;
    assert!(
        other_consumer_result.is_ok(),
        "a different consumer processing the same event must succeed (dedup is per-consumer)"
    );
}

#[tokio::test]
async fn last_processed_returns_most_recent_event_for_consumer() {
    let (pool, _guard) = test_pool().await;
    let inbox = PostgresInboxStore::new(pool.clone());
    let consumer_id = "projector.mission";

    let none_yet = inbox.last_processed(consumer_id).await.unwrap();
    assert!(
        none_yet.is_none(),
        "no last_processed event before any processing"
    );

    let event_1 = EventId(*uuid::Uuid::new_v4().as_bytes());
    let event_2 = EventId(*uuid::Uuid::new_v4().as_bytes());

    inbox
        .mark_processed(consumer_id, &event_1, Timestamp(1_700_000_000_000_000_000))
        .await
        .unwrap();
    inbox
        .mark_processed(consumer_id, &event_2, Timestamp(1_700_000_001_000_000_000))
        .await
        .unwrap();

    let last = inbox.last_processed(consumer_id).await.unwrap();
    assert_eq!(
        last,
        Some(event_2),
        "last_processed should return the most recently processed event"
    );
}
