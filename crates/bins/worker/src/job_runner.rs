use std::{sync::Arc, time::Duration};

use observability_adapter::Metrics;
use platform_kernel::Timestamp;
use rand::Rng;
use serde_json::{json, Value};
use sqlx::PgPool;
use worker_application::{ClaimedJob, JobQueue};

#[derive(Clone, Debug)]
pub struct JobRunnerConfig {
    pub worker_id: String,
    pub batch_size: usize,
    pub lease_seconds: u64,
    pub poll_interval: Duration,
}

impl Default for JobRunnerConfig {
    fn default() -> Self {
        Self {
            worker_id: format!("worker-{}", uuid::Uuid::new_v4()),
            batch_size: 32,
            lease_seconds: 60,
            poll_interval: Duration::from_secs(1),
        }
    }
}

pub fn retry_delay(attempt: u32, jitter_unit: f64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(31);
    let base = 1_u64.checked_shl(exponent).unwrap_or(u64::MAX).min(300);
    let bounded_jitter = jitter_unit.clamp(-1.0, 1.0) * 0.20;
    let millis = ((base as f64 * (1.0 + bounded_jitter)) * 1_000.0) as u64;
    Duration::from_millis(millis.min(300_000))
}

pub async fn run_job_runner(
    queue: Arc<dyn JobQueue>,
    pool: PgPool,
    metrics: Metrics,
    config: JobRunnerConfig,
) -> anyhow::Result<()> {
    loop {
        let recovered = queue.recover_expired_leases(Timestamp::now()).await?;
        if recovered > 0 {
            tracing::warn!(recovered, "recovered expired job leases");
        }
        let jobs = queue
            .claim(&config.worker_id, config.batch_size, config.lease_seconds)
            .await?;
        metrics.job_queue_depth.set(queue.depth().await? as i64);
        if jobs.is_empty() {
            tokio::time::sleep(config.poll_interval).await;
            continue;
        }
        for job in jobs {
            let outcome = execute_job(&pool, &job).await;
            match outcome {
                Ok(()) => {
                    queue.complete(job.id, &job.lease_token).await?;
                    tracing::info!(job_id = job.id.0, job_type = %job.job_type, outcome = "completed", "background job completed");
                }
                Err(error) => {
                    let jitter = rand::thread_rng().gen_range(-1.0_f64..=1.0_f64);
                    let delay = retry_delay(job.attempts, jitter);
                    let retry_at = Timestamp::now()
                        .checked_add(platform_kernel::Duration::from_nanos(
                            delay.as_nanos() as u64
                        ))
                        .unwrap_or(Timestamp(u64::MAX));
                    let failed = queue
                        .fail(job.id, &job.lease_token, &error.to_string(), retry_at)
                        .await?;
                    tracing::warn!(
                        job_id = job.id.0,
                        job_type = %job.job_type,
                        attempts = failed.attempts,
                        dead_lettered = failed.dead_lettered,
                        error_class = "job_execution",
                        "background job failed"
                    );
                }
            }
        }
    }
}

async fn execute_job(pool: &PgPool, job: &ClaimedJob) -> anyhow::Result<()> {
    match job.job_type.as_str() {
        "TimelineTrigger" => execute_timeline_trigger(pool, job).await,
        "SnapshotAggregate" => execute_snapshot_job(pool, job).await,
        other => anyhow::bail!("unsupported job type: {other}"),
    }
}

async fn execute_timeline_trigger(pool: &PgPool, job: &ClaimedJob) -> anyhow::Result<()> {
    let timeline_id = required_string(&job.payload, "timeline_id")?;
    let trigger_kind = required_string(&job.payload, "trigger_kind")?;
    let event_type = match trigger_kind {
        "critical_marker" => "timeline.CriticalMarkerReached",
        "deadline" => "timeline.DeadlineReached",
        "penalty_zone" => "timeline.PenaltyZoneActivated",
        other => anyhow::bail!("unsupported timeline trigger kind: {other}"),
    };
    let organization_id = uuid::Uuid::from_bytes(job.organization_id.0).to_string();
    let now = Timestamp::now();
    let now_ms = (now.0 / 1_000_000) as i64;
    let event_id = uuid::Uuid::new_v4();
    let operation_id = uuid::Uuid::new_v4();
    let correlation_id = uuid::Uuid::new_v4();
    let payload = json!({
        "timeline_id": timeline_id,
        "trigger_kind": trigger_kind,
        "triggered_at_ms": now_ms,
        "job_id": job.id.0,
    });

    let mut tx = pool.begin().await?;
    let version: i64 = sqlx::query_scalar(
        "UPDATE aggregates SET state = jsonb_set(state, '{status}', '\"triggered\"'::jsonb, true), version = version + 1, updated_at = NOW() WHERE id = $1::uuid AND organization_id = $2::uuid AND aggregate_type = 'timeline' RETURNING version",
    )
    .bind(timeline_id)
    .bind(&organization_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO domain_events (event_id, aggregate_id, aggregate_version, event_type, payload, occurred_at, vector_clock, operation_id, correlation_id, causation_id, actor, organization_id) VALUES ($1, $2::uuid, $3, $4, $5, NOW(), '{}'::jsonb, $6, $7, NULL, $8, $9::uuid)",
    )
    .bind(event_id)
    .bind(timeline_id)
    .bind(version)
    .bind(event_type)
    .bind(&payload)
    .bind(operation_id)
    .bind(correlation_id)
    .bind(json!({"service":"worker","actor":"scheduler"}))
    .bind(&organization_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO outbox (event_id, event_type, aggregate_id, organization_id, payload, vector_clock, occurred_at, next_attempt_at) VALUES ($1, $2, $3, $4::uuid, $5, '{}'::jsonb, NOW(), NOW())",
    )
    .bind(event_id)
    .bind(event_type)
    .bind(timeline_id)
    .bind(&organization_id)
    .bind(&payload)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn execute_snapshot_job(pool: &PgPool, job: &ClaimedJob) -> anyhow::Result<()> {
    let aggregate_id = required_string(&job.payload, "aggregate_id")?;
    crate::snapshot_loop::snapshot_aggregate(pool, aggregate_id).await?;
    Ok(())
}

fn required_string<'a>(payload: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("job payload missing string field {key}"))
}
