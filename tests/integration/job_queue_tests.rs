use background_jobs::SqliteJobQueue;
use platform_kernel::{ObjectId, Timestamp};
use sqlx::sqlite::SqlitePoolOptions;
use worker_application::{JobQueue, NewJob};

async fn queue() -> (sqlx::SqlitePool, SqliteJobQueue) {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("sqlite pool");
    sqlx::migrate!("../../migrations/sqlite")
        .run(&pool)
        .await
        .expect("migrations");
    let queue = SqliteJobQueue::new(pool.clone());
    (pool, queue)
}

#[tokio::test]
async fn job_survives_process_restart_lease_expiry_and_reclaim() {
    let path = std::env::temp_dir().join(format!("onyx-team7-jobs-{}.db", uuid::Uuid::new_v4()));
    let url = format!("sqlite://{}?mode=rwc", path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("file-backed sqlite pool");
    sqlx::migrate!("../../migrations/sqlite")
        .run(&pool)
        .await
        .expect("migrations");
    let queue = SqliteJobQueue::new(pool.clone());
    let org = ObjectId([1; 16]);
    let id = queue
        .enqueue(NewJob {
            organization_id: org,
            job_type: "TimelineTrigger".to_string(),
            payload: serde_json::json!({"timeline_id":"x"}),
            next_attempt_at: Timestamp::from_nanos(1),
            max_retries: 10,
            deduplication_key: Some("job-durable".to_string()),
        })
        .await
        .expect("enqueue");
    let first = queue.claim("worker-a", 1, 1).await.expect("claim");
    assert_eq!(first[0].id, id);
    pool.close().await;
    drop(queue);
    drop(pool);

    let restarted_pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .expect("restarted sqlite pool");
    let restarted_queue = SqliteJobQueue::new(restarted_pool.clone());
    restarted_queue
        .recover_expired_leases(Timestamp::from_nanos(u64::MAX - 1))
        .await
        .expect("recover");
    let second = restarted_queue
        .claim("worker-b", 1, 60)
        .await
        .expect("reclaim");
    assert_eq!(second[0].id, id);
    assert_ne!(first[0].lease_token, second[0].lease_token);
    restarted_pool.close().await;
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn failed_job_retries_then_dead_letters_at_max_retries() {
    let (pool, queue) = queue().await;
    queue
        .enqueue(NewJob {
            organization_id: ObjectId([2; 16]),
            job_type: "AlwaysFails".to_string(),
            payload: serde_json::json!({}),
            next_attempt_at: Timestamp::from_nanos(1),
            max_retries: 2,
            deduplication_key: None,
        })
        .await
        .expect("enqueue");

    let first = queue
        .claim("worker", 1, 60)
        .await
        .expect("first claim")
        .remove(0);
    let first_outcome = queue
        .fail(
            first.id,
            &first.lease_token,
            "boom",
            Timestamp::from_nanos(1),
        )
        .await
        .expect("first failure");
    assert!(!first_outcome.dead_lettered);

    let second = queue
        .claim("worker", 1, 60)
        .await
        .expect("second claim")
        .remove(0);
    let second_outcome = queue
        .fail(
            second.id,
            &second.lease_token,
            "boom again",
            Timestamp::from_nanos(1),
        )
        .await
        .expect("second failure");
    assert!(second_outcome.dead_lettered);
    let status: String = sqlx::query_scalar("SELECT status FROM jobs WHERE id = ?")
        .bind(second.id.0)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "dead_letter");
}

#[test]
fn retry_backoff_is_exponential_jittered_and_capped() {
    let low = worker::job_runner::retry_delay(1, -1.0);
    let high = worker::job_runner::retry_delay(1, 1.0);
    assert_eq!(low.as_millis(), 800);
    assert_eq!(high.as_millis(), 1_200);
    assert_eq!(worker::job_runner::retry_delay(20, 1.0).as_secs(), 300);
}
