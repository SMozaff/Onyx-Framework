//! Scheduled scan for `StaffLoan` advance-warning and expiry
//! notifications.
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §2.1, resolved
//! 2026-08-16 — the person confirmed both an advance warning **2-3
//! days before** a loan's `end_at` and a second notification **when it
//! actually ends**, which requires a scheduled background job rather
//! than an on-read check (nothing "reads" a loan proactively at the
//! right moment to notice "this ends in 2 days"). See
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` C.3 for the build guidance
//! this module follows.
//!
//! Deliberately mirrors `scheduler_loop::scheduler_tick_postgres`'s
//! shape exactly (scan `aggregates` for a type/condition, enqueue a
//! job per match) rather than introducing a second, different scanning
//! pattern for one more aggregate type.
//!
//! # Advance-warning "already sent" tracking
//! A loan's advance warning must fire exactly once, not on every scan
//! (`SCHEDULER_INTERVAL` is 5 seconds — a loan sitting in its 2-3 day
//! warning window would otherwise be re-enqueued thousands of times).
//! This is tracked by writing a `advance_warning_sent_at` field onto
//! the `StaffLoan` aggregate's own JSON state the first time the
//! warning job is enqueued for it — not a separate tracking table —
//! since the aggregate's state is the natural place to record "has this
//! happened yet" for a fact intrinsic to the loan itself, and it keeps
//! the scan query simple (`WHERE ... AND advance_warning_sent_at IS
//! NULL`) rather than needing a join against a separate log.

use std::time::Duration;

use chrono::{DateTime, Utc};
use platform_kernel::{ObjectId, Timestamp};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use worker_application::{JobQueue, NewJob};

/// How far in advance of a loan's `end_at` the warning notification
/// fires. Design doc §2.1: "notified two or three days before" — the
/// midpoint, 2.5 days, is used as the single threshold rather than
/// picking one end of the stated range, since the person gave a range
/// rather than a single number and did not indicate either bound was
/// preferred.
pub const ADVANCE_WARNING_LEAD: Duration = Duration::from_secs(60 * 60 * 60); // 2.5 days

/// How often this scan runs. Independent of `scheduler_loop`'s
/// `SCHEDULER_INTERVAL` (5 seconds) — a loan's warning window is
/// measured in days, so a much coarser interval is sufficient and
/// avoids needless load; every 15 minutes comfortably guarantees the
/// warning fires within the stated 2-3 day range without meaningfully
/// risking missing it.
pub const STAFF_LOAN_SCAN_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub async fn run_staff_loan_scheduler(
    queue: std::sync::Arc<dyn JobQueue>,
    pool: PgPool,
) -> anyhow::Result<()> {
    loop {
        let enqueued = staff_loan_scan_tick_postgres(queue.as_ref(), &pool, Utc::now()).await?;
        if enqueued > 0 {
            tracing::info!(enqueued, "staff loan scheduler scan complete");
        } else {
            tracing::debug!(enqueued, "staff loan scheduler scan complete");
        }
        tokio::time::sleep(STAFF_LOAN_SCAN_INTERVAL).await;
    }
}

/// One scan pass. Returns the number of jobs enqueued (advance-warning
/// plus expiry jobs combined). Split from `run_staff_loan_scheduler` so
/// tests can drive a single tick deterministically, matching
/// `scheduler_tick_postgres`'s own precedent.
pub async fn staff_loan_scan_tick_postgres(
    queue: &dyn JobQueue,
    pool: &PgPool,
    now: DateTime<Utc>,
) -> anyhow::Result<u64> {
    let mut enqueued = 0_u64;
    enqueued += enqueue_advance_warnings(queue, pool, now).await?;
    enqueued += enqueue_expirations(queue, pool, now).await?;
    Ok(enqueued)
}

async fn enqueue_advance_warnings(
    queue: &dyn JobQueue,
    pool: &PgPool,
    now: DateTime<Utc>,
) -> anyhow::Result<u64> {
    let warning_threshold = now + chrono::Duration::from_std(ADVANCE_WARNING_LEAD)?;
    // `status` must be Active or Extended (the only states where a loan
    // has a meaningful upcoming end — see
    // `todo_domain::state_machine::StaffLoanStatus`), `end_at` must be
    // at or before the warning threshold (i.e. within the lead window),
    // and no warning has been sent yet.
    let rows = sqlx::query(
        "SELECT id::text AS id, organization_id::text AS organization_id, state \
         FROM aggregates \
         WHERE aggregate_type = 'staff_loan' \
           AND state->>'status' IN ('Active', 'Extended') \
           AND (state->'window'->>'end_at')::bigint <= $1 \
           AND state->>'advance_warning_sent_at' IS NULL",
    )
    .bind(chrono_to_nanos(warning_threshold))
    .fetch_all(pool)
    .await?;

    let mut enqueued = 0_u64;
    for row in rows {
        let id: String = row.try_get("id")?;
        let organization_id: String = row.try_get("organization_id")?;
        let state: Value = row.try_get("state")?;
        enqueue_staff_loan_job(
            queue,
            &id,
            &organization_id,
            &state,
            "StaffLoanAdvanceWarning",
        )
        .await?;
        enqueued += 1;
    }
    Ok(enqueued)
}

async fn enqueue_expirations(
    queue: &dyn JobQueue,
    pool: &PgPool,
    now: DateTime<Utc>,
) -> anyhow::Result<u64> {
    let rows = sqlx::query(
        "SELECT id::text AS id, organization_id::text AS organization_id, state \
         FROM aggregates \
         WHERE aggregate_type = 'staff_loan' \
           AND state->>'status' IN ('Active', 'Extended') \
           AND (state->'window'->>'end_at')::bigint <= $1",
    )
    .bind(chrono_to_nanos(now))
    .fetch_all(pool)
    .await?;

    let mut enqueued = 0_u64;
    for row in rows {
        let id: String = row.try_get("id")?;
        let organization_id: String = row.try_get("organization_id")?;
        let state: Value = row.try_get("state")?;
        enqueue_staff_loan_job(queue, &id, &organization_id, &state, "StaffLoanExpiry").await?;
        enqueued += 1;
    }
    Ok(enqueued)
}

async fn enqueue_staff_loan_job(
    queue: &dyn JobQueue,
    staff_loan_id: &str,
    organization_id: &str,
    state: &Value,
    job_type: &str,
) -> anyhow::Result<()> {
    let organization_uuid = uuid::Uuid::parse_str(organization_id)?;
    let staff_user_id = state.get("staff_user_id").cloned().unwrap_or(Value::Null);
    let real_owner_id = state.get("real_owner_id").cloned().unwrap_or(Value::Null);
    let borrowing_manager_id = state
        .get("borrowing_manager_id")
        .cloned()
        .unwrap_or(Value::Null);
    queue
        .enqueue(NewJob {
            organization_id: ObjectId(*organization_uuid.as_bytes()),
            job_type: job_type.to_string(),
            payload: json!({
                "staff_loan_id": staff_loan_id,
                "staff_user_id": staff_user_id,
                "real_owner_id": real_owner_id,
                "borrowing_manager_id": borrowing_manager_id,
            }),
            next_attempt_at: Timestamp::now(),
            max_retries: 10,
            // Deliberately does NOT include a timestamp component (unlike
            // `enqueue_timeline_rows`'s dedup key) — this job type is
            // meant to fire at most once per loan ever (advance warning)
            // or once per loan (expiry, which also flips `status` away
            // from Active/Extended so the scan naturally stops
            // rematching it), so keying purely on
            // `job_type:staff_loan_id` is the correct dedup boundary.
            deduplication_key: Some(format!("{job_type}:{staff_loan_id}")),
        })
        .await?;
    Ok(())
}

fn chrono_to_nanos(dt: DateTime<Utc>) -> i64 {
    dt.timestamp_nanos_opt().unwrap_or(i64::MAX)
}

#[cfg(test)]
mod postgres_integration_tests {
    use std::collections::BTreeSet;

    use super::*;
    use background_jobs::PostgresJobQueue;
    use sqlx::postgres::PgPoolOptions;
    use worker_application::JobQueue;

    /// Executes the scheduler and its two staff-loan job handlers against a
    /// real PostgreSQL database when `DATABASE_URL` is supplied. The test is
    /// intentionally a no-op for ordinary developer runs without PostgreSQL;
    /// CI or an explicit live-Postgres run supplies the connection URL.
    #[tokio::test]
    async fn staff_loan_warning_and_expiry_round_trip_on_postgres() -> anyhow::Result<()> {
        let Ok(database_url) = std::env::var("DATABASE_URL") else {
            eprintln!(
                "skipping live PostgreSQL staff-loan integration test: DATABASE_URL is unset"
            );
            return Ok(());
        };
        if !database_url.starts_with("postgres") {
            eprintln!("skipping live PostgreSQL staff-loan integration test: DATABASE_URL is not PostgreSQL");
            return Ok(());
        }

        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(&database_url)
            .await?;
        let queue = PostgresJobQueue::new(pool.clone());
        let organization_id = uuid::Uuid::new_v4();
        let staff_loan_id = uuid::Uuid::new_v4();
        let staff_user_id = uuid::Uuid::new_v4();
        let real_owner_id = uuid::Uuid::new_v4();
        let borrowing_manager_id = uuid::Uuid::new_v4();
        let now = Utc::now();
        let future_end_at = chrono_to_nanos(now + chrono::Duration::days(1));

        let state = json!({
            "public_id": staff_loan_id.to_string(),
            "status": "Active",
            "staff_user_id": staff_user_id.as_bytes(),
            "real_owner_id": real_owner_id.as_bytes(),
            "borrowing_manager_id": borrowing_manager_id.as_bytes(),
            "window": {
                "start_at": chrono_to_nanos(now - chrono::Duration::days(1)),
                "end_at": future_end_at,
            },
        });
        sqlx::query(
            "INSERT INTO aggregates (id, aggregate_type, organization_id, version, lifecycle_epoch, authority_epoch, state, updated_at) \
             VALUES ($1, 'staff_loan', $2, 1, 0, 0, $3, NOW())",
        )
        .bind(staff_loan_id)
        .bind(organization_id)
        .bind(&state)
        .execute(&pool)
        .await?;

        // A loan ending within the 2.5-day window receives exactly one
        // durable advance-warning job.
        assert_eq!(staff_loan_scan_tick_postgres(&queue, &pool, now).await?, 1);
        let warning_jobs: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM jobs \
             WHERE organization_id = $1 AND job_type = 'StaffLoanAdvanceWarning'",
        )
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(warning_jobs, 1);

        let mut claimed = queue.claim("staff-loan-postgres-test", 8, 60).await?;
        assert_eq!(claimed.len(), 1);
        let warning_job = claimed.pop().expect("one warning job was claimed");
        assert_eq!(warning_job.job_type, "StaffLoanAdvanceWarning");
        crate::job_runner::execute_staff_loan_advance_warning(&pool, &warning_job).await?;
        queue
            .complete(warning_job.id, &warning_job.lease_token)
            .await?;

        // The JSONB update must add a new top-level key to a state object
        // that did not contain it before executing the warning handler.
        let warning_sent_at: Option<i64> = sqlx::query_scalar(
            "SELECT (state->>'advance_warning_sent_at')::bigint \
             FROM aggregates WHERE id = $1 AND organization_id = $2",
        )
        .bind(staff_loan_id)
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert!(warning_sent_at.is_some());

        let warning_notifications: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM aggregates \
             WHERE organization_id = $1 AND aggregate_type = 'notification' \
               AND state->>'title' = 'Staff loan ending soon'",
        )
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(warning_notifications, 3);
        let expected_recipients = BTreeSet::from([
            staff_user_id.to_string(),
            real_owner_id.to_string(),
            borrowing_manager_id.to_string(),
        ]);
        let warning_recipients: BTreeSet<String> = sqlx::query_scalar(
            "SELECT state->>'recipient_id' FROM aggregates \
             WHERE organization_id = $1 AND aggregate_type = 'notification' \
               AND state->>'title' = 'Staff loan ending soon'",
        )
        .bind(organization_id)
        .fetch_all(&pool)
        .await?
        .into_iter()
        .collect();
        assert_eq!(warning_recipients, expected_recipients);

        // The persisted marker prevents a second scheduler tick from
        // enqueuing another advance-warning job.
        assert_eq!(staff_loan_scan_tick_postgres(&queue, &pool, now).await?, 0);
        let warning_jobs_after_second_tick: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM jobs \
             WHERE organization_id = $1 AND job_type = 'StaffLoanAdvanceWarning'",
        )
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(warning_jobs_after_second_tick, 1);

        // Move the same active loan into the past and drive the expiry path.
        let past_end_at = chrono_to_nanos(now - chrono::Duration::days(1));
        sqlx::query(
            "UPDATE aggregates \
             SET state = jsonb_set(state, '{window,end_at}', to_jsonb($3::bigint), true) \
             WHERE id = $1 AND organization_id = $2",
        )
        .bind(staff_loan_id)
        .bind(organization_id)
        .bind(past_end_at)
        .execute(&pool)
        .await?;

        assert_eq!(staff_loan_scan_tick_postgres(&queue, &pool, now).await?, 1);
        let mut claimed = queue.claim("staff-loan-postgres-test", 8, 60).await?;
        assert_eq!(claimed.len(), 1);
        let expiry_job = claimed.pop().expect("one expiry job was claimed");
        assert_eq!(expiry_job.job_type, "StaffLoanExpiry");
        crate::job_runner::execute_staff_loan_expiry(&pool, &expiry_job).await?;
        queue
            .complete(expiry_job.id, &expiry_job.lease_token)
            .await?;

        let status: String = sqlx::query_scalar(
            "SELECT state->>'status' FROM aggregates WHERE id = $1 AND organization_id = $2",
        )
        .bind(staff_loan_id)
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(status, "Expired");

        let expiry_events: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM domain_events \
             WHERE aggregate_id = $1 AND organization_id = $2 \
               AND event_type = 'staff_loan.StaffLoanExpired'",
        )
        .bind(staff_loan_id)
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(expiry_events, 1);

        let expiry_outbox: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM outbox \
             WHERE aggregate_id = $1::text AND organization_id = $2 \
               AND event_type = 'staff_loan.StaffLoanExpired'",
        )
        .bind(staff_loan_id)
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(expiry_outbox, 1);

        let expiry_notifications: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM aggregates \
             WHERE organization_id = $1 AND aggregate_type = 'notification' \
               AND state->>'title' = 'Staff loan ended'",
        )
        .bind(organization_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(expiry_notifications, 3);
        let expiry_recipients: BTreeSet<String> = sqlx::query_scalar(
            "SELECT state->>'recipient_id' FROM aggregates \
             WHERE organization_id = $1 AND aggregate_type = 'notification' \
               AND state->>'title' = 'Staff loan ended'",
        )
        .bind(organization_id)
        .fetch_all(&pool)
        .await?
        .into_iter()
        .collect();
        assert_eq!(expiry_recipients, expected_recipients);

        Ok(())
    }
}
