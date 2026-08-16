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

pub async fn run_staff_loan_scheduler(queue: std::sync::Arc<dyn JobQueue>, pool: PgPool) -> anyhow::Result<()> {
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
    let staff_user_id = state
        .get("staff_user_id")
        .cloned()
        .unwrap_or(Value::Null);
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
