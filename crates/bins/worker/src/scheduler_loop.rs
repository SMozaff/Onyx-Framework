use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use platform_kernel::{ObjectId, Timestamp};
use serde_json::{json, Value};
use sqlx::{PgPool, Row, SqlitePool};
use worker_application::{JobQueue, NewJob};

pub const SCHEDULER_INTERVAL: Duration = Duration::from_secs(5);

pub async fn run_scheduler(queue: Arc<dyn JobQueue>, pool: PgPool) -> anyhow::Result<()> {
    loop {
        let enqueued = scheduler_tick_postgres(queue.as_ref(), &pool, Utc::now()).await?;
        tracing::debug!(enqueued, "timeline scheduler scan complete");
        tokio::time::sleep(SCHEDULER_INTERVAL).await;
    }
}

pub async fn scheduler_tick_postgres(
    queue: &dyn JobQueue,
    pool: &PgPool,
    now: DateTime<Utc>,
) -> anyhow::Result<u64> {
    let rows = sqlx::query(
        "SELECT id::text AS id, organization_id::text AS organization_id, state FROM aggregates WHERE aggregate_type = 'timeline' AND state->>'status' = 'upcoming' AND (state->>'at')::timestamptz <= $1",
    )
    .bind(now)
    .fetch_all(pool)
    .await?;
    enqueue_timeline_rows(
        queue,
        rows.into_iter()
            .map(|row| -> anyhow::Result<(String, String, Value)> {
                let id: String = row.try_get("id")?;
                let organization_id: String = row.try_get("organization_id")?;
                let state: Value = row.try_get("state")?;
                Ok((id, organization_id, state))
            }),
    )
    .await
}

pub async fn scheduler_tick_sqlite(
    queue: &dyn JobQueue,
    pool: &SqlitePool,
    now: DateTime<Utc>,
) -> anyhow::Result<u64> {
    let rows = sqlx::query(
        "SELECT id, organization_id, state FROM aggregates WHERE aggregate_type = 'timeline'",
    )
    .fetch_all(pool)
    .await?;
    let mut candidates = Vec::new();
    for row in rows {
        let id: Vec<u8> = row.try_get("id")?;
        let organization_id: Vec<u8> = row.try_get("organization_id")?;
        let state_raw: String = row.try_get("state")?;
        let state: Value = serde_json::from_str(&state_raw)?;
        if state.get("status").and_then(Value::as_str) != Some("upcoming") {
            continue;
        }
        let at = state
            .get("at")
            .and_then(Value::as_str)
            .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
            .map(|value| value.with_timezone(&Utc));
        if at.map(|value| value <= now).unwrap_or(false) {
            candidates.push(Ok((
                uuid::Uuid::from_slice(&id)?.to_string(),
                uuid::Uuid::from_slice(&organization_id)?.to_string(),
                state,
            )));
        }
    }
    enqueue_timeline_rows(queue, candidates).await
}

async fn enqueue_timeline_rows<I>(queue: &dyn JobQueue, rows: I) -> anyhow::Result<u64>
where
    I: IntoIterator<Item = Result<(String, String, Value), anyhow::Error>>,
{
    let mut enqueued = 0_u64;
    for row in rows {
        let (timeline_id, organization_id, state) = row?;
        let kind = state
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or("deadline");
        let at = state.get("at").and_then(Value::as_str).unwrap_or_default();
        let organization_uuid = uuid::Uuid::parse_str(&organization_id)?;
        queue
            .enqueue(NewJob {
                organization_id: ObjectId(*organization_uuid.as_bytes()),
                job_type: "TimelineTrigger".to_string(),
                payload: json!({
                    "timeline_id": timeline_id,
                    "trigger_kind": kind,
                    "scheduled_at": at,
                }),
                next_attempt_at: Timestamp::now(),
                max_retries: 10,
                deduplication_key: Some(format!("timeline:{timeline_id}:{kind}:{at}")),
            })
            .await?;
        enqueued = enqueued.saturating_add(1);
    }
    Ok(enqueued)
}
