use background_jobs::SqliteJobQueue;
use chrono::{Duration, Utc};
use sqlx::sqlite::SqlitePoolOptions;
use worker::scheduler_loop::scheduler_tick_sqlite;
use worker::snapshot_loop::snapshot_tick_sqlite;
use worker_application::JobQueue;

#[tokio::test]
async fn scheduler_enqueues_due_timeline_trigger_once() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../../migrations/sqlite")
        .run(&pool)
        .await
        .unwrap();
    let timeline_id = uuid::Uuid::new_v4();
    let org = uuid::Uuid::new_v4();
    let due_at = (Utc::now() - Duration::seconds(5)).to_rfc3339();
    sqlx::query("INSERT INTO aggregates (id, aggregate_type, version, lifecycle_epoch, authority_epoch, state, updated_at, organization_id) VALUES (?, 'timeline', 1, 0, 0, ?, 0, ?)")
        .bind(timeline_id.as_bytes().to_vec())
        .bind(serde_json::json!({"status":"upcoming","kind":"deadline","at":due_at}).to_string())
        .bind(org.as_bytes().to_vec())
        .execute(&pool).await.unwrap();
    let queue = SqliteJobQueue::new(pool.clone());
    assert_eq!(
        scheduler_tick_sqlite(&queue, &pool, Utc::now())
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        scheduler_tick_sqlite(&queue, &pool, Utc::now())
            .await
            .unwrap(),
        1
    );
    assert_eq!(queue.depth().await.unwrap(), 1);
}

#[tokio::test]
async fn snapshotter_creates_snapshot_after_threshold() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::migrate!("../../migrations/sqlite")
        .run(&pool)
        .await
        .unwrap();
    let aggregate = uuid::Uuid::new_v4();
    let org = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO aggregates (id, aggregate_type, version, lifecycle_epoch, authority_epoch, state, updated_at, organization_id) VALUES (?, 'mission', 1001, 0, 0, '{}', 0, ?)")
        .bind(aggregate.as_bytes().to_vec()).bind(org.as_bytes().to_vec()).execute(&pool).await.unwrap();
    for version in 1..=1001_i64 {
        sqlx::query("INSERT INTO domain_events (event_id, aggregate_id, aggregate_version, event_type, payload, occurred_at, vector_clock, operation_id, correlation_id, causation_id, actor, organization_id) VALUES (?, ?, ?, 'test.Event', '{}', 0, '{}', ?, ?, NULL, '{}', ?)")
            .bind(uuid::Uuid::new_v4().as_bytes().to_vec())
            .bind(aggregate.as_bytes().to_vec())
            .bind(version)
            .bind(uuid::Uuid::new_v4().as_bytes().to_vec())
            .bind(uuid::Uuid::new_v4().as_bytes().to_vec())
            .bind(org.as_bytes().to_vec())
            .execute(&pool).await.unwrap();
    }
    assert_eq!(snapshot_tick_sqlite(&pool).await.unwrap(), 1);
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM aggregate_snapshots")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
}
