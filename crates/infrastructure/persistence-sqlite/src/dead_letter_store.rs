//! SQLite `DeadLetterStore` implementation. Sole writer of the
//! `dead_letter` table per Architectural Ruling D1.

use async_trait::async_trait;
use persistence_common::value_to_text;
use sqlx::SqlitePool;
use worker_application::{ClaimedMessage, DeadLetterError, DeadLetterStore};

pub struct SqliteDeadLetterStore {
    pool: SqlitePool,
}

impl SqliteDeadLetterStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DeadLetterStore for SqliteDeadLetterStore {
    async fn send(&self, claimed: &ClaimedMessage, error: &str) -> Result<(), DeadLetterError> {
        let event_id_bytes = claimed.message.event_id.0.to_vec();
        let organization_id_bytes = claimed.message.organization_id.0.to_vec();
        let now_millis = persistence_common::timestamp_to_millis(platform_kernel::Timestamp::now());

        let payload_value: serde_json::Value = serde_json::from_slice(&claimed.message.payload)
            .map_err(|e| DeadLetterError::Serialization(e.to_string()))?;
        let payload_text = value_to_text(&payload_value)
            .map_err(|e| DeadLetterError::Serialization(e.to_string()))?;

        sqlx::query(
            r#"
            INSERT INTO dead_letter (outbox_id, event_id, payload, last_error, failed_at, organization_id)
            VALUES (?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(claimed.outbox_id.0 as i64)
        .bind(event_id_bytes)
        .bind(payload_text)
        .bind(error)
        .bind(now_millis)
        .bind(organization_id_bytes)
        .execute(&self.pool)
        .await
        .map_err(|e| DeadLetterError::Database(e.to_string()))?;

        Ok(())
    }
}
