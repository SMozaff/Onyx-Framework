//! Integration test: `PostgresOutboxStore` full lifecycle against a live
//! PostgreSQL database. One of the 4 required integration test suites per
//! Team Prompt 2 §2 File Manifest ("outbox" suite).

use persistence_postgres::{PostgresOutboxStore, PostgresUnitOfWork};
use platform_kernel::{Duration, EventId, ObjectId, OrganizationId, Timestamp, VectorClock};
use query_application::{OutboxMessage, UnitOfWork};
use sqlx::postgres::PgPoolOptions;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use worker_application::{OutboxError, OutboxStore};

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

fn sample_message(organization_id: OrganizationId) -> OutboxMessage {
    OutboxMessage {
        event_id: EventId(*uuid::Uuid::new_v4().as_bytes()),
        event_type: "MissionCreated".to_string(),
        aggregate_id: uuid::Uuid::new_v4().to_string(),
        organization_id,
        payload: serde_json::to_vec(&serde_json::json!({ "name": "Test Mission" })).unwrap(),
        vector_clock: VectorClock::new(),
        occurred_at: Timestamp::now(),
    }
}

#[tokio::test]
async fn claim_publish_and_pending_count_lifecycle() {
    let (pool, _guard) = test_pool().await;
    let org = ObjectId(*uuid::Uuid::new_v4().as_bytes());

    let mut uow = PostgresUnitOfWork::begin(&pool, org).await.unwrap();
    let message = sample_message(org);
    uow.register_outbox(message.clone());
    uow.commit()
        .await
        .expect("commit should persist the outbox row");

    let outbox_store = PostgresOutboxStore::new(pool.clone());

    let pending = outbox_store.pending_count().await.unwrap();
    assert_eq!(pending, 1, "one message should be pending after commit");

    let claimed = outbox_store
        .claim_unpublished("relay-1", 10)
        .await
        .expect("claim should succeed");
    assert_eq!(
        claimed.len(),
        1,
        "should claim exactly the one pending message"
    );
    assert_eq!(claimed[0].message.event_type, "MissionCreated");
    assert_eq!(claimed[0].retry_count, 0);

    // A second claim attempt (different consumer) should see nothing —
    // the row is under an active lease.
    let second_claim = outbox_store.claim_unpublished("relay-2", 10).await.unwrap();
    assert!(
        second_claim.is_empty(),
        "leased row should not be claimable again while lease is active"
    );

    outbox_store
        .mark_published(claimed[0].outbox_id)
        .await
        .expect("mark_published should succeed");

    let pending_after = outbox_store.pending_count().await.unwrap();
    assert_eq!(
        pending_after, 0,
        "no messages should be pending after publish"
    );
}

#[tokio::test]
async fn mark_failed_increments_retry_and_schedules_retry() {
    let (pool, _guard) = test_pool().await;
    let org = ObjectId(*uuid::Uuid::new_v4().as_bytes());

    let mut uow = PostgresUnitOfWork::begin(&pool, org).await.unwrap();
    uow.register_outbox(sample_message(org));
    uow.commit().await.unwrap();

    let outbox_store = PostgresOutboxStore::new(pool.clone());

    let claimed = outbox_store.claim_unpublished("relay-1", 10).await.unwrap();
    assert_eq!(claimed.len(), 1);

    outbox_store
        .mark_failed(
            claimed[0].outbox_id,
            "simulated transient failure",
            Duration::from_secs(0),
        )
        .await
        .expect("mark_failed should succeed");

    // With retry_after = 0s, the row should be immediately re-claimable
    // (next_attempt_at <= NOW()), with retry_count incremented.
    let reclaimed = outbox_store.claim_unpublished("relay-1", 10).await.unwrap();
    assert_eq!(
        reclaimed.len(),
        1,
        "row should be reclaimable after mark_failed with zero backoff"
    );
    assert_eq!(
        reclaimed[0].retry_count, 1,
        "retry_count should have incremented"
    );
}

#[tokio::test]
async fn dead_letter_moves_message_out_of_pending_and_preserves_audit_row() {
    let (pool, _guard) = test_pool().await;
    let org = ObjectId(*uuid::Uuid::new_v4().as_bytes());

    let mut uow = PostgresUnitOfWork::begin(&pool, org).await.unwrap();
    let message = sample_message(org);
    uow.register_outbox(message.clone());
    uow.commit().await.unwrap();

    let outbox_store = PostgresOutboxStore::new(pool.clone());
    let claimed = outbox_store.claim_unpublished("relay-1", 10).await.unwrap();
    assert_eq!(claimed.len(), 1);

    // Per Architectural Ruling "Dead Letter Table Single Writer"
    // (DECISIONS.md D1): OutboxStore::dead_letter() is the only call a
    // relay needs to make — it internally delegates to DeadLetterStore
    // as the sole writer of the dead_letter table, then flips the
    // outbox row's own status. Calling DeadLetterStore::send()
    // separately after this would (correctly) produce a second row —
    // that is not this test's job to exercise; PostgresDeadLetterStore
    // is unit-level plumbing consumed internally by PostgresOutboxStore.
    outbox_store
        .dead_letter(claimed[0].outbox_id, "permanently failed per Ruling 3b")
        .await
        .expect("dead_letter should succeed");

    let pending = outbox_store.pending_count().await.unwrap();
    assert_eq!(
        pending, 0,
        "dead-lettered message should no longer be pending"
    );

    // Original row preserved for audit (Team Prompt 2 §4.1: "Never delete
    // published rows immediately").
    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        row_count, 1,
        "original outbox row must be preserved, not deleted"
    );

    // Exactly one dead_letter row should exist — proving DeadLetterStore
    // is the sole writer and there is no duplicate insert from
    // OutboxStore::dead_letter() itself.
    let dl_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM dead_letter")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        dl_count, 1,
        "dead_letter table should have exactly one row (single-writer design, DECISIONS.md D1)"
    );
}

/// Regression test for ruling V1 (`DECISIONS.md`): a non-empty
/// `VectorClock` previously failed to serialize to JSON at all
/// (`platform_kernel::VectorClock`'s derived `Serialize` cannot encode
/// non-string `ReplicaId` map keys), and `insert_outbox_message` masked
/// that failure by silently writing `null` instead of the real value. V1
/// fixed both: `VectorClock` now has a custom hex-string-keyed
/// `Serialize`/`Deserialize`, and the outbox insert now propagates a
/// genuine serialization failure instead of swallowing it. This test
/// proves the fix holds through the real, live Postgres round-trip — not
/// just at the `VectorClock` unit level (already covered in
/// `platform-kernel`'s own tests) — by seeding a vector clock with real
/// entries and asserting the exact counts survive commit + claim.
#[tokio::test]
async fn non_empty_vector_clock_round_trips_through_real_postgres_outbox() {
    let (pool, _guard) = test_pool().await;
    let org = ObjectId(*uuid::Uuid::new_v4().as_bytes());

    let replica_a = platform_kernel::ReplicaId::new_random();
    let replica_b = platform_kernel::ReplicaId::new_random();
    let mut vector_clock = VectorClock::new();
    vector_clock.increment(replica_a);
    vector_clock.increment(replica_a);
    vector_clock.increment(replica_b);

    let mut message = sample_message(org);
    message.vector_clock = vector_clock.clone();

    let mut uow = PostgresUnitOfWork::begin(&pool, org).await.unwrap();
    uow.register_outbox(message);
    uow.commit().await.expect(
        "commit must succeed for a non-empty vector clock (previously failed before ruling V1)",
    );

    let outbox_store = PostgresOutboxStore::new(pool.clone());
    let claimed = outbox_store
        .claim_unpublished("relay-1", 10)
        .await
        .expect("claim should succeed");

    assert_eq!(claimed.len(), 1);
    assert_eq!(
        claimed[0].message.vector_clock, vector_clock,
        "the vector clock read back must exactly match what was written, not a silently-emptied default"
    );
    assert_eq!(claimed[0].message.vector_clock.get(&replica_a), 2);
    assert_eq!(claimed[0].message.vector_clock.get(&replica_b), 1);
}

/// Regression test for ruling V2 (`DECISIONS.md`): the read-side
/// deserialization fallback (`unwrap_or_default()`) previously turned any
/// row whose stored `vector_clock` didn't match `VectorClock`'s shape
/// into a silently-empty clock — indistinguishable from a genuinely empty
/// one. V2 replaced that with error propagation. This test writes a row
/// via `sample_message` (so it passes every other column's constraints),
/// then directly overwrites its `vector_clock` column with syntactically
/// valid JSON that does not match `VectorClock`'s shape — the real
/// "corrupt row" scenario the `JSONB` column type still permits (it
/// enforces valid JSON syntax, not our application-level shape) — and
/// asserts `claim_unpublished` returns `OutboxError::Serialization`
/// rather than a claimed message with a silently-empty clock.
#[tokio::test]
async fn claim_unpublished_returns_an_error_for_a_malformed_vector_clock_column() {
    let (pool, _guard) = test_pool().await;
    let org = ObjectId(*uuid::Uuid::new_v4().as_bytes());

    let mut uow = PostgresUnitOfWork::begin(&pool, org).await.unwrap();
    uow.register_outbox(sample_message(org));
    uow.commit().await.unwrap();

    // Overwrite the just-committed row's vector_clock with valid JSON
    // that is the wrong shape for VectorClock (entries values are not
    // u64-parseable, and the key is not a 32-char hex ReplicaId).
    sqlx::query("UPDATE outbox SET vector_clock = $1::jsonb")
        .bind(serde_json::json!({"entries": {"not-a-valid-replica-id": "not-a-number"}}))
        .execute(&pool)
        .await
        .expect("direct corruption of the test row should succeed");

    let outbox_store = PostgresOutboxStore::new(pool.clone());
    let result = outbox_store.claim_unpublished("relay-1", 10).await;

    match result {
        Err(OutboxError::Serialization(msg)) => {
            assert!(
                msg.contains("vector_clock") || !msg.is_empty(),
                "error message should be informative, got: {msg}"
            );
        }
        other => {
            panic!("expected Err(OutboxError::Serialization(_)) for a corrupt row, got {other:?}")
        }
    }
}
