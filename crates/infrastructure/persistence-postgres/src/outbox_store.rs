//! PostgreSQL `OutboxStore` implementation.
//! Port defined per Team Prompt 2 §3.3 (verbatim).

use async_trait::async_trait;
use persistence_common::{timestamp_to_millis, uuid_to_bytes};
use platform_kernel::{Duration, Timestamp, VectorClock};
use query_application::{OutboxId, OutboxMessage};
use sqlx::PgPool;
use worker_application::{ClaimedMessage, DeadLetterStore, LeaseToken, OutboxError, OutboxStore};

pub struct PostgresOutboxStore {
    pool: PgPool,
    dead_letter_store: crate::dead_letter_store::PostgresDeadLetterStore,
}

impl PostgresOutboxStore {
    pub fn new(pool: PgPool) -> Self {
        let dead_letter_store =
            crate::dead_letter_store::PostgresDeadLetterStore::new(pool.clone());
        Self {
            pool,
            dead_letter_store,
        }
    }
}

#[async_trait]
impl OutboxStore for PostgresOutboxStore {
    async fn claim_unpublished(
        &self,
        consumer_id: &str,
        limit: usize,
    ) -> Result<Vec<ClaimedMessage>, OutboxError> {
        // Per Team Prompt 2 §3.3/§5.1: a lease prevents duplicate
        // processing across concurrent relay workers. We generate a fresh
        // lease token per claim batch and set a lease window; only rows
        // that are unpublished, due for retry (next_attempt_at <= now),
        // and not currently under an unexpired lease are eligible.
        // `FOR UPDATE SKIP LOCKED` prevents two relay workers from
        // claiming the same row concurrently — this is our own addition
        // since Team Prompt 2 doesn't specify the claim SQL; documented
        // as DECISIONS.md item P3.
        let lease_token = uuid::Uuid::new_v4();
        let lease_duration_millis: i64 = 30_000; // 30s lease window; see DECISIONS.md P3.
        let now = Timestamp::now();
        let now_millis = timestamp_to_millis(now);
        let lease_expires_millis = now_millis + lease_duration_millis;

        let mut txn = self
            .pool
            .begin()
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        let rows = sqlx::query!(
            r#"
            SELECT outbox_id, event_id, event_type, aggregate_id, organization_id,
                   payload, vector_clock, occurred_at, retry_count
            FROM outbox
            WHERE published = FALSE
              AND next_attempt_at <= NOW()
              AND (lease_expires_at IS NULL OR lease_expires_at <= NOW())
            ORDER BY next_attempt_at ASC
            LIMIT $1
            FOR UPDATE SKIP LOCKED
            "#,
            limit as i64,
        )
        .fetch_all(&mut *txn)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;

        if rows.is_empty() {
            txn.commit()
                .await
                .map_err(|e| OutboxError::Database(e.to_string()))?;
            return Ok(Vec::new());
        }

        let ids: Vec<i64> = rows.iter().map(|r| r.outbox_id).collect();

        sqlx::query!(
            r#"
            UPDATE outbox
            SET lease_token = $1, lease_expires_at = to_timestamp($2::double precision / 1000.0)
            WHERE outbox_id = ANY($3)
            "#,
            lease_token,
            lease_expires_millis as f64,
            &ids,
        )
        .execute(&mut *txn)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;

        txn.commit()
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        let mut claimed = Vec::with_capacity(rows.len());
        for row in rows {
            let event_id_bytes = uuid_to_bytes(row.event_id);
            let organization_id_bytes = uuid_to_bytes(row.organization_id);
            let occurred_at_millis = row.occurred_at.timestamp_millis();
            // Ruling V2 (DECISIONS.md): previously `.unwrap_or_default()`,
            // silently returning an empty VectorClock for any row whose
            // stored vector_clock JSON failed to deserialize — a corrupt
            // row would be indistinguishable from a genuinely empty
            // clock. Propagated as a hard error instead; a corrupt row
            // is deterministic and should surface, not default.
            let vector_clock: VectorClock = serde_json::from_value(row.vector_clock)
                .map_err(|e| OutboxError::Serialization(e.to_string()))?;

            claimed.push(ClaimedMessage {
                outbox_id: OutboxId(row.outbox_id as u64),
                message: OutboxMessage {
                    event_id: platform_kernel::EventId(event_id_bytes),
                    event_type: row.event_type,
                    aggregate_id: row.aggregate_id,
                    organization_id: platform_kernel::ObjectId(organization_id_bytes),
                    payload: serde_json::to_vec(&row.payload).unwrap_or_default(),
                    vector_clock,
                    occurred_at: platform_kernel::Timestamp(
                        (occurred_at_millis as u64).saturating_mul(1_000_000),
                    ),
                },
                lease_token: LeaseToken(lease_token.to_string()),
                retry_count: row.retry_count as u32,
            });
        }

        let _ = consumer_id; // consumer_id is not persisted per-row in the frozen §4.1 schema (no consumer_group column); accepted for interface compatibility. See DECISIONS.md item P4.

        Ok(claimed)
    }

    async fn mark_published(&self, outbox_id: OutboxId) -> Result<(), OutboxError> {
        sqlx::query!(
            "UPDATE outbox SET published = TRUE, lease_token = NULL, lease_expires_at = NULL WHERE outbox_id = $1",
            outbox_id.0 as i64,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(())
    }

    async fn mark_failed(
        &self,
        outbox_id: OutboxId,
        error: &str,
        retry_after: Duration,
    ) -> Result<(), OutboxError> {
        let retry_after_millis = (retry_after.0 / 1_000_000) as i64;
        sqlx::query!(
            r#"
            UPDATE outbox
            SET retry_count = retry_count + 1,
                last_error = $1,
                lease_token = NULL,
                lease_expires_at = NULL,
                next_attempt_at = NOW() + ($2 || ' milliseconds')::interval
            WHERE outbox_id = $3
            "#,
            error,
            retry_after_millis.to_string(),
            outbox_id.0 as i64,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(())
    }

    async fn dead_letter(&self, outbox_id: OutboxId, error: &str) -> Result<(), OutboxError> {
        // Per Architectural Ruling ("Dead Letter Table Single Writer" —
        // DECISIONS.md D1): this method does NOT write to the dead_letter
        // table directly. It reconstructs the ClaimedMessage from the
        // outbox row, delegates the actual dead_letter insert to
        // DeadLetterStore::send (the sole writer), then marks the outbox
        // row published so it is never reclaimed again.
        let row = sqlx::query!(
            r#"
            SELECT event_id, event_type, aggregate_id, organization_id, payload,
                   vector_clock, occurred_at, retry_count
            FROM outbox
            WHERE outbox_id = $1
            "#,
            outbox_id.0 as i64,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;

        let Some(row) = row else {
            return Err(OutboxError::Database(format!(
                "outbox row {} not found for dead-lettering",
                outbox_id.0
            )));
        };

        let event_id_bytes = uuid_to_bytes(row.event_id);
        let organization_id_bytes = uuid_to_bytes(row.organization_id);
        let occurred_at_millis = row.occurred_at.timestamp_millis();
        // Ruling V2 (DECISIONS.md): see the matching comment in
        // claim_unpublished above — same fix, same rationale.
        let vector_clock: VectorClock = serde_json::from_value(row.vector_clock)
            .map_err(|e| OutboxError::Serialization(e.to_string()))?;

        let claimed = ClaimedMessage {
            outbox_id,
            message: OutboxMessage {
                event_id: platform_kernel::EventId(event_id_bytes),
                event_type: row.event_type,
                aggregate_id: row.aggregate_id,
                organization_id: platform_kernel::ObjectId(organization_id_bytes),
                payload: serde_json::to_vec(&row.payload).unwrap_or_default(),
                vector_clock,
                occurred_at: platform_kernel::Timestamp(
                    (occurred_at_millis as u64).saturating_mul(1_000_000),
                ),
            },
            lease_token: LeaseToken(String::new()),
            retry_count: row.retry_count as u32,
        };

        self.dead_letter_store
            .send(&claimed, error)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        // Per Team Prompt 2 §4.1 comment: "Never delete published rows
        // immediately; retain per audit policy." We interpret this as
        // applying equally to dead-lettered rows: mark published=TRUE (it
        // will never be retried again) rather than deleting, retaining it
        // for audit alongside the new dead_letter row. See DECISIONS.md P5.
        sqlx::query!(
            "UPDATE outbox SET published = TRUE, last_error = $1 WHERE outbox_id = $2",
            error,
            outbox_id.0 as i64,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;

        Ok(())
    }

    async fn pending_count(&self) -> Result<u64, OutboxError> {
        let row = sqlx::query!("SELECT COUNT(*) as count FROM outbox WHERE published = FALSE")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(row.count.unwrap_or(0) as u64)
    }
}
