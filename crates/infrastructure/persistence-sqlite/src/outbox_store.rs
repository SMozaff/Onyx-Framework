//! SQLite `OutboxStore` implementation. Mirrors `PostgresOutboxStore`
//! with SQLite column conventions (S1): BLOB for UUIDs, TEXT for JSON,
//! INTEGER (millis) for timestamps, INTEGER (0/1) for booleans.

use async_trait::async_trait;
use persistence_common::{text_to_value, timestamp_to_millis, value_to_text};
use platform_kernel::{Duration, ObjectId, VectorClock};
use query_application::{OutboxId, OutboxMessage};
use sqlx::SqlitePool;
use worker_application::{ClaimedMessage, DeadLetterStore, LeaseToken, OutboxError, OutboxStore};

pub struct SqliteOutboxStore {
    pool: SqlitePool,
    dead_letter_store: crate::dead_letter_store::SqliteDeadLetterStore,
}

impl SqliteOutboxStore {
    pub fn new(pool: SqlitePool) -> Self {
        let dead_letter_store = crate::dead_letter_store::SqliteDeadLetterStore::new(pool.clone());
        Self {
            pool,
            dead_letter_store,
        }
    }
}

#[async_trait]
impl OutboxStore for SqliteOutboxStore {
    async fn claim_unpublished(
        &self,
        _consumer_id: &str,
        limit: usize,
    ) -> Result<Vec<ClaimedMessage>, OutboxError> {
        // SQLite note: no FOR UPDATE SKIP LOCKED — SQLite uses
        // database-level locking instead. For the single-writer desktop/
        // offline use case this adapter targets, that is acceptable; see
        // DECISIONS.md item P3a (SQLite concurrency model).
        let lease_token = uuid::Uuid::new_v4().to_string();
        let now_millis = timestamp_to_millis(platform_kernel::Timestamp::now());
        let lease_expires_millis = now_millis + 30_000i64;

        let mut txn = self
            .pool
            .begin()
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        let rows = sqlx::query_as::<
            _,
            (
                i64,
                Vec<u8>,
                String,
                String,
                Vec<u8>,
                String,
                String,
                i64,
                i64,
            ),
        >(
            r#"
            SELECT outbox_id, event_id, event_type, aggregate_id, organization_id,
                   payload, vector_clock, occurred_at, retry_count
            FROM outbox
            WHERE published = 0
              AND next_attempt_at <= ?
              AND (lease_expires_at IS NULL OR lease_expires_at <= ?)
            ORDER BY next_attempt_at ASC
            LIMIT ?
            "#,
        )
        .bind(now_millis)
        .bind(now_millis)
        .bind(limit as i64)
        .fetch_all(&mut *txn)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;

        if rows.is_empty() {
            txn.commit()
                .await
                .map_err(|e| OutboxError::Database(e.to_string()))?;
            return Ok(Vec::new());
        }

        let ids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        for id in &ids {
            sqlx::query(
                "UPDATE outbox SET lease_token = ?, lease_expires_at = ? WHERE outbox_id = ?",
            )
            .bind(&lease_token)
            .bind(lease_expires_millis)
            .bind(id)
            .execute(&mut *txn)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;
        }

        txn.commit()
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        let mut claimed = Vec::with_capacity(rows.len());
        for (
            outbox_id,
            event_id_bytes,
            event_type,
            aggregate_id,
            organization_id_bytes,
            payload_text,
            vector_clock_text,
            occurred_at_millis,
            retry_count,
        ) in rows
        {
            // Ruling V2 (DECISIONS.md): previously an `.ok().and_then(...)
            // .unwrap_or_default()` chain, silently returning an empty
            // VectorClock if EITHER the stored TEXT column failed to
            // parse as JSON or the parsed JSON failed to deserialize into
            // a VectorClock — a corrupt row was indistinguishable from a
            // genuinely empty clock. Propagated as a hard error instead,
            // preserving which of the two stages failed in the message.
            let vector_clock_value = text_to_value(&vector_clock_text).map_err(|e| {
                OutboxError::Serialization(format!(
                    "outbox row {outbox_id}: vector_clock column was not valid JSON text: {e}"
                ))
            })?;
            let vector_clock: VectorClock =
                serde_json::from_value(vector_clock_value).map_err(|e| {
                    OutboxError::Serialization(format!(
                        "outbox row {outbox_id}: vector_clock JSON did not match VectorClock: {e}"
                    ))
                })?;
            let payload = text_to_value(&payload_text)
                .ok()
                .map(|v| serde_json::to_vec(&v).unwrap_or_default())
                .unwrap_or_default();

            let mut event_id_arr = [0u8; 16];
            let mut org_id_arr = [0u8; 16];
            let n = 16.min(event_id_bytes.len());
            event_id_arr[..n].copy_from_slice(&event_id_bytes[..n]);
            let n = 16.min(organization_id_bytes.len());
            org_id_arr[..n].copy_from_slice(&organization_id_bytes[..n]);

            claimed.push(ClaimedMessage {
                outbox_id: OutboxId(outbox_id as u64),
                message: OutboxMessage {
                    event_id: platform_kernel::EventId(event_id_arr),
                    event_type,
                    aggregate_id,
                    organization_id: ObjectId(org_id_arr),
                    payload,
                    vector_clock,
                    occurred_at: platform_kernel::Timestamp(
                        (occurred_at_millis as u64).saturating_mul(1_000_000),
                    ),
                },
                lease_token: LeaseToken(lease_token.clone()),
                retry_count: retry_count as u32,
            });
        }

        Ok(claimed)
    }

    async fn mark_published(&self, outbox_id: OutboxId) -> Result<(), OutboxError> {
        sqlx::query("UPDATE outbox SET published = 1, lease_token = NULL, lease_expires_at = NULL WHERE outbox_id = ?")
            .bind(outbox_id.0 as i64)
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
        let now_millis = timestamp_to_millis(platform_kernel::Timestamp::now());
        sqlx::query(
            r#"
            UPDATE outbox
            SET retry_count = retry_count + 1,
                last_error = ?,
                lease_token = NULL,
                lease_expires_at = NULL,
                next_attempt_at = ?
            WHERE outbox_id = ?
            "#,
        )
        .bind(error)
        .bind(now_millis + retry_after_millis)
        .bind(outbox_id.0 as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(())
    }

    async fn dead_letter(&self, outbox_id: OutboxId, error: &str) -> Result<(), OutboxError> {
        // Same delegation pattern as PostgresOutboxStore per ruling D1:
        // DeadLetterStore is the sole writer of the dead_letter table.
        let row =
            sqlx::query_as::<_, (Vec<u8>, String, String, Vec<u8>, String, String, i64, i64)>(
                r#"
            SELECT event_id, event_type, aggregate_id, organization_id,
                   payload, vector_clock, occurred_at, retry_count
            FROM outbox WHERE outbox_id = ?
            "#,
            )
            .bind(outbox_id.0 as i64)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        let Some((
            event_id_bytes,
            event_type,
            aggregate_id,
            org_bytes,
            payload_text,
            vector_clock_text,
            occurred_at_millis,
            retry_count,
        )) = row
        else {
            return Err(OutboxError::Database(format!(
                "outbox row {} not found for dead-lettering",
                outbox_id.0
            )));
        };

        // Ruling V2 (DECISIONS.md): see the matching comment in
        // claim_unpublished above — same fix, same rationale.
        let vector_clock_value = text_to_value(&vector_clock_text).map_err(|e| {
            OutboxError::Serialization(format!(
                "outbox row {}: vector_clock column was not valid JSON text: {e}",
                outbox_id.0
            ))
        })?;
        let vector_clock: VectorClock =
            serde_json::from_value(vector_clock_value).map_err(|e| {
                OutboxError::Serialization(format!(
                    "outbox row {}: vector_clock JSON did not match VectorClock: {e}",
                    outbox_id.0
                ))
            })?;
        let payload = text_to_value(&payload_text)
            .ok()
            .map(|v| serde_json::to_vec(&v).unwrap_or_default())
            .unwrap_or_default();

        let mut event_id_arr = [0u8; 16];
        let mut org_arr = [0u8; 16];
        let n = 16.min(event_id_bytes.len());
        event_id_arr[..n].copy_from_slice(&event_id_bytes[..n]);
        let n = 16.min(org_bytes.len());
        org_arr[..n].copy_from_slice(&org_bytes[..n]);

        let claimed = ClaimedMessage {
            outbox_id,
            message: OutboxMessage {
                event_id: platform_kernel::EventId(event_id_arr),
                event_type,
                aggregate_id,
                organization_id: ObjectId(org_arr),
                payload,
                vector_clock,
                occurred_at: platform_kernel::Timestamp(
                    (occurred_at_millis as u64).saturating_mul(1_000_000),
                ),
            },
            lease_token: LeaseToken(String::new()),
            retry_count: retry_count as u32,
        };

        self.dead_letter_store
            .send(&claimed, error)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        sqlx::query("UPDATE outbox SET published = 1, last_error = ? WHERE outbox_id = ?")
            .bind(error)
            .bind(outbox_id.0 as i64)
            .execute(&self.pool)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;

        let _ = value_to_text; // satisfies clippy; referenced indirectly via persistence-common
        Ok(())
    }

    async fn pending_count(&self) -> Result<u64, OutboxError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM outbox WHERE published = 0")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| OutboxError::Database(e.to_string()))?;
        Ok(count as u64)
    }
}
