//! SQLite `InboxStore` implementation.

use async_trait::async_trait;
use persistence_common::timestamp_to_millis;
use platform_kernel::{EventId, Timestamp};
use sqlx::SqlitePool;
use worker_application::{InboxError, InboxStore};

pub struct SqliteInboxStore {
    pool: SqlitePool,
}

impl SqliteInboxStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InboxStore for SqliteInboxStore {
    async fn is_processed(
        &self,
        consumer_id: &str,
        event_id: &EventId,
    ) -> Result<bool, InboxError> {
        let event_id_bytes = event_id.0.to_vec();
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM inbox WHERE consumer_id = ? AND event_id = ?")
                .bind(consumer_id)
                .bind(&event_id_bytes)
                .fetch_one(&self.pool)
                .await
                .map_err(|e| InboxError::Database(e.to_string()))?;
        Ok(count > 0)
    }

    async fn mark_processed(
        &self,
        consumer_id: &str,
        event_id: &EventId,
        processed_at: Timestamp,
    ) -> Result<(), InboxError> {
        let event_id_bytes = event_id.0.to_vec();
        let processed_at_millis = timestamp_to_millis(processed_at);

        let result = sqlx::query(
            r#"
            INSERT INTO inbox (consumer_id, event_id, processed_at)
            VALUES (?, ?, ?)
            ON CONFLICT (consumer_id, event_id) DO NOTHING
            "#,
        )
        .bind(consumer_id)
        .bind(&event_id_bytes)
        .bind(processed_at_millis)
        .execute(&self.pool)
        .await
        .map_err(|e| InboxError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(InboxError::AlreadyProcessed);
        }
        Ok(())
    }

    async fn last_processed(&self, consumer_id: &str) -> Result<Option<EventId>, InboxError> {
        let row: Option<Vec<u8>> = sqlx::query_scalar(
            r#"
            SELECT event_id FROM inbox
            WHERE consumer_id = ?
            ORDER BY processed_at DESC
            LIMIT 1
            "#,
        )
        .bind(consumer_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| InboxError::Database(e.to_string()))?;

        Ok(row.map(|bytes| {
            let mut arr = [0u8; 16];
            let n = 16.min(bytes.len());
            arr[..n].copy_from_slice(&bytes[..n]);
            EventId(arr)
        }))
    }
}
