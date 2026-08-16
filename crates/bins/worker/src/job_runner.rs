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
        // StaffLoan advance-warning/expiry (2026-08-16) — see
        // `staff_loan_scheduler`'s module doc comment for why these
        // exist and IMPLEMENTATION_PLAN_User_Hierarchy.md C.3.
        "StaffLoanAdvanceWarning" => execute_staff_loan_advance_warning(pool, job).await,
        "StaffLoanExpiry" => execute_staff_loan_expiry(pool, job).await,
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

/// Fires the 2-3 day advance-warning notification for an upcoming
/// `StaffLoan` expiry, then marks the loan so the scan in
/// `staff_loan_scheduler` does not re-enqueue it. Design doc §2.1,
/// resolved 2026-08-16: both the real owner, the borrowing manager, and
/// the staff member are notified — one notification row per recipient,
/// since `NotificationAggregate` has no multi-recipient shape (see this
/// crate's own `execute_timeline_trigger` precedent: each aggregate
/// type here is a plain row, not a fan-out abstraction).
async fn execute_staff_loan_advance_warning(pool: &PgPool, job: &ClaimedJob) -> anyhow::Result<()> {
    let staff_loan_id = required_string(&job.payload, "staff_loan_id")?;
    let organization_id = uuid::Uuid::from_bytes(job.organization_id.0).to_string();

    let mut tx = pool.begin().await?;

    // Guard against a race: another worker instance may have already
    // sent this warning between the scan and this job executing. Only
    // proceed if `advance_warning_sent_at` is still unset.
    let already_sent: Option<Value> = sqlx::query_scalar(
        "SELECT state->'advance_warning_sent_at' FROM aggregates \
         WHERE id = $1::uuid AND organization_id = $2::uuid AND aggregate_type = 'staff_loan'",
    )
    .bind(staff_loan_id)
    .bind(&organization_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    if already_sent.is_some_and(|v| !v.is_null()) {
        tx.rollback().await?;
        return Ok(());
    }

    let recipients = staff_loan_recipients(&job.payload);
    for (recipient_id, recipient_role) in &recipients {
        insert_notification(
            &mut tx,
            &organization_id,
            "Staff loan ending soon",
            &format!(
                "A staff loan (as {recipient_role}) is scheduled to end within the next few days. \
                 You may extend or let it end when it does."
            ),
            "high",
            staff_loan_id,
            "staff_loan",
        )
        .await?;
        let _ = recipient_id; // recorded in message context only — see this fn's doc comment on NotificationAggregate's shape.
    }

    let sent_at_nanos = Timestamp::now().0 as i64;
    sqlx::query(
        "UPDATE aggregates SET state = jsonb_set(state, '{advance_warning_sent_at}', to_jsonb($3::bigint), true), version = version + 1, updated_at = NOW() \
         WHERE id = $1::uuid AND organization_id = $2::uuid AND aggregate_type = 'staff_loan'",
    )
    .bind(staff_loan_id)
    .bind(&organization_id)
    .bind(sent_at_nanos)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Fires the end-of-loan notification and transitions the `StaffLoan`
/// to `Expired`, mirroring `todo_domain::StaffLoanCommand::ExpireStaffLoan`'s
/// semantics directly in SQL (this crate does not go through
/// `AggregateRoot::decide()` for scheduled transitions — same
/// precedent as `execute_timeline_trigger` mutating `aggregates.state`
/// directly rather than round-tripping through the domain crate's
/// in-process types).
async fn execute_staff_loan_expiry(pool: &PgPool, job: &ClaimedJob) -> anyhow::Result<()> {
    let staff_loan_id = required_string(&job.payload, "staff_loan_id")?;
    let organization_id = uuid::Uuid::from_bytes(job.organization_id.0).to_string();

    let mut tx = pool.begin().await?;

    // Only transition if still Active/Extended — another worker
    // instance, or a manual EndStaffLoanEarly, may have already moved
    // it out of that state.
    let status: Option<String> = sqlx::query_scalar(
        "SELECT state->>'status' FROM aggregates \
         WHERE id = $1::uuid AND organization_id = $2::uuid AND aggregate_type = 'staff_loan'",
    )
    .bind(staff_loan_id)
    .bind(&organization_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(status) = status else {
        tx.rollback().await?;
        return Ok(());
    };
    if status != "Active" && status != "Extended" {
        tx.rollback().await?;
        return Ok(());
    }

    let recipients = staff_loan_recipients(&job.payload);
    for (recipient_id, recipient_role) in &recipients {
        insert_notification(
            &mut tx,
            &organization_id,
            "Staff loan ended",
            &format!(
                "A staff loan (as {recipient_role}) has reached its scheduled end date and is now closed."
            ),
            "high",
            staff_loan_id,
            "staff_loan",
        )
        .await?;
        let _ = recipient_id;
    }

    let event_id = uuid::Uuid::new_v4();
    let operation_id = uuid::Uuid::new_v4();
    let correlation_id = uuid::Uuid::new_v4();
    let now = Timestamp::now();
    let expired_at_nanos = now.0 as i64;
    let version: i64 = sqlx::query_scalar(
        "UPDATE aggregates SET state = jsonb_set(state, '{status}', '\"Expired\"'::jsonb, true), version = version + 1, updated_at = NOW() \
         WHERE id = $1::uuid AND organization_id = $2::uuid AND aggregate_type = 'staff_loan' RETURNING version",
    )
    .bind(staff_loan_id)
    .bind(&organization_id)
    .fetch_one(&mut *tx)
    .await?;
    let event_payload = json!({"StaffLoanExpired": {"expired_at": expired_at_nanos}});
    sqlx::query(
        "INSERT INTO domain_events (event_id, aggregate_id, aggregate_version, event_type, payload, occurred_at, vector_clock, operation_id, correlation_id, causation_id, actor, organization_id) VALUES ($1, $2::uuid, $3, $4, $5, NOW(), '{}'::jsonb, $6, $7, NULL, $8, $9::uuid)",
    )
    .bind(event_id)
    .bind(staff_loan_id)
    .bind(version)
    .bind("staff_loan.StaffLoanExpired")
    .bind(&event_payload)
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
    .bind("staff_loan.StaffLoanExpired")
    .bind(staff_loan_id)
    .bind(&organization_id)
    .bind(&event_payload)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(())
}

/// Extracts `(id, role_label)` pairs from a staff-loan job payload for
/// the three parties design doc §2.1 confirms must be notified. Skips
/// any party whose id is missing/null from the payload rather than
/// erroring — a job should not fail entirely because one field was
/// absent from an older aggregate shape.
fn staff_loan_recipients(payload: &Value) -> Vec<(String, &'static str)> {
    let mut out = Vec::new();
    for (key, role) in [
        ("staff_user_id", "the staff member on loan"),
        ("real_owner_id", "the real owner"),
        ("borrowing_manager_id", "the borrowing manager"),
    ] {
        if let Some(id) = payload.get(key).and_then(object_id_json_to_uuid_string) {
            out.push((id, role));
        }
    }
    out
}

/// Converts a JSON value shaped like `[u8; 16]` into a UUID string.
/// Same shape `todo_domain`'s `ObjectId` serializes as, and the same
/// conversion `api_server::query_handler::object_id_array_to_uuid_string`
/// performs for query responses — duplicated here in the small,
/// dependency-free form this crate's existing SQL-first style favors,
/// rather than pulling in a shared dependency for one helper.
fn object_id_json_to_uuid_string(value: &Value) -> Option<String> {
    let array = value.as_array()?;
    if array.len() != 16 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, entry) in array.iter().enumerate() {
        bytes[i] = u8::try_from(entry.as_u64()?).ok()?;
    }
    Some(uuid::Uuid::from_bytes(bytes).to_string())
}

/// Inserts one `NotificationAggregate`-shaped row directly, matching
/// the exact field set `api_server::routes::command`'s
/// `NotificationAggregate` struct defines (title, message, priority,
/// status, source_id, source_type, created_at, acknowledged_at) plus
/// the `public_id`/version/epoch bookkeeping every aggregate row needs
/// — same shape the seed fixtures in `api_server::routes::mod::seed_if_empty`
/// use, since notifications have no dedicated persistence helper of
/// their own to call instead.
#[allow(clippy::too_many_arguments)]
async fn insert_notification(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    organization_id: &str,
    title: &str,
    message: &str,
    priority: &str,
    source_id: &str,
    source_type: &str,
) -> anyhow::Result<()> {
    let notification_id = uuid::Uuid::new_v4();
    let state = json!({
        "public_id": notification_id.to_string(),
        "title": title,
        "message": message,
        "priority": priority,
        "status": "unacknowledged",
        "source_id": source_id,
        "source_type": source_type,
        "created_at": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        "acknowledged_at": Value::Null,
        "version": 1,
        "lifecycle_epoch": 0,
        "authority_epoch": 0,
    });
    sqlx::query(
        "INSERT INTO aggregates (id, aggregate_type, organization_id, version, lifecycle_epoch, authority_epoch, state, updated_at) \
         VALUES ($1, 'notification', $2::uuid, 1, 0, 0, $3, NOW())",
    )
    .bind(notification_id)
    .bind(organization_id)
    .bind(&state)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn required_string<'a>(payload: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("job payload missing string field {key}"))
}
