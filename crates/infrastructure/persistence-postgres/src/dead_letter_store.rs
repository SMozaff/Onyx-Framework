//! PostgreSQL `DeadLetterStore` implementation.
//! Port defined per Architectural Ruling (Team 2 Kickoff, item 4).
//!
//! Distinct from `OutboxStore::dead_letter()` (which flips the outbox
//! row's own status): this is the separate durable sink the relay loop
//! (§5.1, per Architectural Ruling 3b) writes to for a permanently-failed
//! claimed message.

use async_trait::async_trait;
use persistence_common::{bytes_to_uuid, timestamp_to_millis};
use sqlx::PgPool;
use worker_application::{ClaimedMessage, DeadLetterError, DeadLetterStore};

pub struct PostgresDeadLetterStore {
    pool: PgPool,
}

impl PostgresDeadLetterStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl DeadLetterStore for PostgresDeadLetterStore {
    async fn send(&self, claimed: &ClaimedMessage, error: &str) -> Result<(), DeadLetterError> {
        let event_id_uuid = bytes_to_uuid(claimed.message.event_id.0);
        let organization_id_uuid = bytes_to_uuid(claimed.message.organization_id.0);
        let _ = timestamp_to_millis(claimed.message.occurred_at); // available if the schema needs it; dead_letter table records failed_at = NOW() instead (see §4.1)

        let payload_json: serde_json::Value = serde_json::from_slice(&claimed.message.payload)
            .map_err(|e| DeadLetterError::Serialization(e.to_string()))?;

        sqlx::query!(
            r#"
            INSERT INTO dead_letter (outbox_id, event_id, payload, last_error, organization_id)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            claimed.outbox_id.0 as i64,
            event_id_uuid,
            payload_json,
            error,
            organization_id_uuid,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| DeadLetterError::Database(e.to_string()))?;

        Ok(())
    }
}
