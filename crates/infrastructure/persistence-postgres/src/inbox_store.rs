//! PostgreSQL `InboxStore` implementation.
//! Port defined per Team Prompt 2 §3.4 (verbatim).

use async_trait::async_trait;
use persistence_common::{bytes_to_uuid, uuid_to_bytes};
use platform_kernel::{EventId, Timestamp};
use sqlx::PgPool;
use worker_application::{InboxError, InboxStore};

pub struct PostgresInboxStore {
    pool: PgPool,
}

impl PostgresInboxStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InboxStore for PostgresInboxStore {
    async fn is_processed(
        &self,
        consumer_id: &str,
        event_id: &EventId,
    ) -> Result<bool, InboxError> {
        let event_id_uuid = bytes_to_uuid(event_id.0);
        let row = sqlx::query!(
            "SELECT 1 as one FROM inbox WHERE consumer_id = $1 AND event_id = $2",
            consumer_id,
            event_id_uuid,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| InboxError::Database(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn mark_processed(
        &self,
        consumer_id: &str,
        event_id: &EventId,
        processed_at: Timestamp,
    ) -> Result<(), InboxError> {
        let event_id_uuid = bytes_to_uuid(event_id.0);
        let processed_at_millis = persistence_common::timestamp_to_millis(processed_at);

        let result = sqlx::query!(
            r#"
            INSERT INTO inbox (consumer_id, event_id, processed_at)
            VALUES ($1, $2, to_timestamp($3::double precision / 1000.0))
            ON CONFLICT (consumer_id, event_id) DO NOTHING
            "#,
            consumer_id,
            event_id_uuid,
            processed_at_millis as f64,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| InboxError::Database(e.to_string()))?;

        // Per the port contract (§3.4), a second call for an already-
        // processed (consumer_id, event_id) pair should be reported as
        // AlreadyProcessed rather than silently succeeding, so callers can
        // distinguish "first time processing this event" from "duplicate
        // delivery, already handled" for their own logging/metrics. The
        // INSERT itself is idempotent (ON CONFLICT DO NOTHING) so the
        // underlying data is correct either way. See DECISIONS.md item P6.
        if result.rows_affected() == 0 {
            return Err(InboxError::AlreadyProcessed);
        }

        Ok(())
    }

    async fn last_processed(&self, consumer_id: &str) -> Result<Option<EventId>, InboxError> {
        let row = sqlx::query!(
            r#"
            SELECT event_id FROM inbox
            WHERE consumer_id = $1
            ORDER BY processed_at DESC
            LIMIT 1
            "#,
            consumer_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| InboxError::Database(e.to_string()))?;

        Ok(row.map(|r| EventId(uuid_to_bytes(r.event_id))))
    }
}
