//! `TokenRevocationStore` implementations. See
//! `security_application::ports::token_revocation` for why two operations
//! (single-token and per-user-watermark) exist and what each is for.

use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use security_application::{TokenRevocationError, TokenRevocationStore};
use sqlx::PgPool;
use tokio::sync::Mutex;

/// Real, shared, durable store — every replica reads and writes the same
/// Postgres tables, so a revocation performed on one is visible to all
/// immediately. This is the only implementation permitted in production;
/// `ApiState::new` selects it whenever a Postgres pool (governance or
/// primary) is available, mirroring `PostgresSlidingWindowRateLimiter`'s
/// own selection precedent.
#[derive(Clone)]
pub struct PostgresTokenRevocationStore {
    pool: PgPool,
}

impl PostgresTokenRevocationStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TokenRevocationStore for PostgresTokenRevocationStore {
    async fn revoke_token(
        &self,
        token_hash: &str,
        revoked_at: u64,
    ) -> Result<(), TokenRevocationError> {
        sqlx::query(
            "INSERT INTO revoked_tokens (token_hash, revoked_at) VALUES ($1, $2) \
             ON CONFLICT (token_hash) DO NOTHING",
        )
        .bind(token_hash)
        .bind(revoked_at as i64)
        .execute(&self.pool)
        .await
        .map_err(|error| TokenRevocationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn is_token_revoked(&self, token_hash: &str) -> Result<bool, TokenRevocationError> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT token_hash FROM revoked_tokens WHERE token_hash = $1")
                .bind(token_hash)
                .fetch_optional(&self.pool)
                .await
                .map_err(|error| TokenRevocationError::Infrastructure(error.to_string()))?;
        Ok(row.is_some())
    }

    async fn revoke_all_for_user(
        &self,
        user_id: &str,
        revoked_before: u64,
    ) -> Result<(), TokenRevocationError> {
        // GREATEST(...) keeps the watermark monotonic under concurrent
        // writers (see the port's doc comment) — a slightly-stale
        // `revoked_before` can never roll an existing, more recent
        // watermark backwards.
        sqlx::query(
            "INSERT INTO user_token_revocations (user_id, revoked_before) VALUES ($1::uuid, $2) \
             ON CONFLICT (user_id) DO UPDATE SET \
             revoked_before = GREATEST(user_token_revocations.revoked_before, EXCLUDED.revoked_before)",
        )
        .bind(user_id)
        .bind(revoked_before as i64)
        .execute(&self.pool)
        .await
        .map_err(|error| TokenRevocationError::Infrastructure(error.to_string()))?;
        Ok(())
    }

    async fn user_revoked_before(
        &self,
        user_id: &str,
    ) -> Result<Option<u64>, TokenRevocationError> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT revoked_before FROM user_token_revocations WHERE user_id = $1::uuid",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| TokenRevocationError::Infrastructure(error.to_string()))?;
        Ok(row.map(|(value,)| value.max(0) as u64))
    }
}

/// Deterministic development/test fallback for a pure-SQLite, single-
/// instance deployment with no governance Postgres and no Postgres
/// primary database configured. Carries exactly the limitation the
/// former `ApiState::revoked_tokens` field had everywhere (in-process,
/// non-durable, invisible to any other replica) — now scoped honestly to
/// only the topology where that limitation is actually harmless: one
/// process, no horizontal scaling. Production composition MUST use
/// `PostgresTokenRevocationStore` — `ApiState::new` enforces this
/// indirectly, since `ONYX_ENV=production` already requires a Postgres
/// primary and `ONYX_GOVERNANCE_DATABASE_URL`.
#[derive(Clone, Default)]
pub struct InMemoryTokenRevocationStore {
    revoked_tokens: Arc<Mutex<HashSet<String>>>,
    user_watermarks: Arc<Mutex<HashMap<String, u64>>>,
}

#[async_trait]
impl TokenRevocationStore for InMemoryTokenRevocationStore {
    async fn revoke_token(
        &self,
        token_hash: &str,
        _revoked_at: u64,
    ) -> Result<(), TokenRevocationError> {
        self.revoked_tokens
            .lock()
            .await
            .insert(token_hash.to_string());
        Ok(())
    }

    async fn is_token_revoked(&self, token_hash: &str) -> Result<bool, TokenRevocationError> {
        Ok(self.revoked_tokens.lock().await.contains(token_hash))
    }

    async fn revoke_all_for_user(
        &self,
        user_id: &str,
        revoked_before: u64,
    ) -> Result<(), TokenRevocationError> {
        let mut watermarks = self.user_watermarks.lock().await;
        let entry = watermarks.entry(user_id.to_string()).or_insert(0);
        *entry = (*entry).max(revoked_before);
        Ok(())
    }

    async fn user_revoked_before(
        &self,
        user_id: &str,
    ) -> Result<Option<u64>, TokenRevocationError> {
        Ok(self.user_watermarks.lock().await.get(user_id).copied())
    }
}
