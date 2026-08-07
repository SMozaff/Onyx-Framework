//! Postgres and SQLite implementations of the [`UserStore`] port.
//!
//! # Provenance
//! Audit finding **H-01**. Two concrete types rather than one generic over
//! `sqlx::Database`: that mirrors the split this project already uses
//! (`persistence-postgres` / `persistence-sqlite`) and avoids the lifetime and
//! type-inference friction of writing database-generic sqlx queries for a
//! surface this small.
//!
//! # Invariants upheld here
//! * Usernames are lowercased on **write** and on **lookup**, matching the
//!   `LOWER(username)` unique index in both migrations.
//! * A unique-constraint violation is mapped to
//!   [`UserStoreError::DuplicateUsername`] rather than a generic infrastructure
//!   error, so the API can answer 409 instead of 500.
//! * Nothing here logs a password hash.

use async_trait::async_trait;
use security_application::{NewUser, UserRecord, UserStore, UserStoreError};
use sqlx::{PgPool, Row, SqlitePool};

fn infrastructure(error: sqlx::Error) -> UserStoreError {
    UserStoreError::Infrastructure(error.to_string())
}

/// Maps a sqlx error to `DuplicateUsername` when it is a unique-constraint
/// violation, and to `Infrastructure` otherwise.
///
/// Postgres reports this as SQLSTATE 23505. SQLite has no SQLSTATE, so its
/// driver surfaces the constraint through the message; both are handled.
fn insert_error(error: sqlx::Error) -> UserStoreError {
    if let sqlx::Error::Database(db) = &error {
        if db.code().as_deref() == Some("23505") {
            return UserStoreError::DuplicateUsername;
        }
        let message = db.message().to_ascii_lowercase();
        if message.contains("unique") {
            return UserStoreError::DuplicateUsername;
        }
    }
    infrastructure(error)
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

// ---------------------------------------------------------------- Postgres --

#[derive(Clone)]
pub struct PostgresUserStore {
    pool: PgPool,
}

impl PostgresUserStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn record_from_pg_row(row: &sqlx::postgres::PgRow) -> Result<UserRecord, UserStoreError> {
    let id: uuid::Uuid = row.try_get("id").map_err(infrastructure)?;
    let organization_id: uuid::Uuid = row.try_get("organization_id").map_err(infrastructure)?;
    Ok(UserRecord {
        user_id: id.to_string(),
        username: row.try_get("username").map_err(infrastructure)?,
        organization_id: organization_id.to_string(),
        password_hash: row.try_get("password_hash").map_err(infrastructure)?,
        is_admin: row.try_get("is_admin").map_err(infrastructure)?,
        is_active: row.try_get("is_active").map_err(infrastructure)?,
    })
}

fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, UserStoreError> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| UserStoreError::Infrastructure(format!("{field} is not a valid UUID")))
}

#[async_trait]
impl UserStore for PostgresUserStore {
    async fn find_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserRecord>, UserStoreError> {
        let row = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_active \
             FROM users WHERE LOWER(username) = $1",
        )
        .bind(username.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(infrastructure)?;
        row.as_ref().map(record_from_pg_row).transpose()
    }

    async fn find_by_id(&self, user_id: &str) -> Result<Option<UserRecord>, UserStoreError> {
        let row = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_active \
             FROM users WHERE id = $1",
        )
        .bind(parse_uuid(user_id, "user_id")?)
        .fetch_optional(&self.pool)
        .await
        .map_err(infrastructure)?;
        row.as_ref().map(record_from_pg_row).transpose()
    }

    async fn create(&self, user: NewUser) -> Result<UserRecord, UserStoreError> {
        sqlx::query(
            "INSERT INTO users (id, username, organization_id, password_hash, is_admin) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(parse_uuid(&user.user_id, "user_id")?)
        .bind(user.username.to_lowercase())
        .bind(parse_uuid(&user.organization_id, "organization_id")?)
        .bind(&user.password_hash)
        .bind(user.is_admin)
        .execute(&self.pool)
        .await
        .map_err(insert_error)?;
        Ok(UserRecord {
            user_id: user.user_id,
            username: user.username.to_lowercase(),
            organization_id: user.organization_id,
            password_hash: user.password_hash,
            is_admin: user.is_admin,
            is_active: true,
        })
    }

    async fn list(&self) -> Result<Vec<UserRecord>, UserStoreError> {
        let rows = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_active \
             FROM users ORDER BY username",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(infrastructure)?;
        rows.iter().map(record_from_pg_row).collect()
    }

    async fn set_active(&self, user_id: &str, is_active: bool) -> Result<(), UserStoreError> {
        let result = sqlx::query(
            "UPDATE users SET is_active = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(parse_uuid(user_id, "user_id")?)
        .bind(is_active)
        .execute(&self.pool)
        .await
        .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn set_password_hash(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> Result<(), UserStoreError> {
        let result = sqlx::query(
            "UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(parse_uuid(user_id, "user_id")?)
        .bind(password_hash)
        .execute(&self.pool)
        .await
        .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn count(&self) -> Result<u64, UserStoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(infrastructure)?;
        Ok(count.max(0) as u64)
    }
}

// ------------------------------------------------------------------ SQLite --

#[derive(Clone)]
pub struct SqliteUserStore {
    pool: SqlitePool,
}

impl SqliteUserStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn record_from_sqlite_row(row: &sqlx::sqlite::SqliteRow) -> Result<UserRecord, UserStoreError> {
    // SQLite stores the booleans as INTEGER (see the migration), so they are
    // read as i64 and normalised here rather than relying on driver coercion.
    let is_admin: i64 = row.try_get("is_admin").map_err(infrastructure)?;
    let is_active: i64 = row.try_get("is_active").map_err(infrastructure)?;
    Ok(UserRecord {
        user_id: row.try_get("id").map_err(infrastructure)?,
        username: row.try_get("username").map_err(infrastructure)?,
        organization_id: row.try_get("organization_id").map_err(infrastructure)?,
        password_hash: row.try_get("password_hash").map_err(infrastructure)?,
        is_admin: is_admin != 0,
        is_active: is_active != 0,
    })
}

#[async_trait]
impl UserStore for SqliteUserStore {
    async fn find_by_username(
        &self,
        username: &str,
    ) -> Result<Option<UserRecord>, UserStoreError> {
        let row = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_active \
             FROM users WHERE LOWER(username) = ?1",
        )
        .bind(username.to_lowercase())
        .fetch_optional(&self.pool)
        .await
        .map_err(infrastructure)?;
        row.as_ref().map(record_from_sqlite_row).transpose()
    }

    async fn find_by_id(&self, user_id: &str) -> Result<Option<UserRecord>, UserStoreError> {
        let row = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_active \
             FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(infrastructure)?;
        row.as_ref().map(record_from_sqlite_row).transpose()
    }

    async fn create(&self, user: NewUser) -> Result<UserRecord, UserStoreError> {
        let now = now_millis();
        sqlx::query(
            "INSERT INTO users (id, username, organization_id, password_hash, is_admin, is_active, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
        )
        .bind(&user.user_id)
        .bind(user.username.to_lowercase())
        .bind(&user.organization_id)
        .bind(&user.password_hash)
        .bind(i64::from(user.is_admin))
        .bind(now)
        .execute(&self.pool)
        .await
        .map_err(insert_error)?;
        Ok(UserRecord {
            user_id: user.user_id,
            username: user.username.to_lowercase(),
            organization_id: user.organization_id,
            password_hash: user.password_hash,
            is_admin: user.is_admin,
            is_active: true,
        })
    }

    async fn list(&self) -> Result<Vec<UserRecord>, UserStoreError> {
        let rows = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_active \
             FROM users ORDER BY username",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(infrastructure)?;
        rows.iter().map(record_from_sqlite_row).collect()
    }

    async fn set_active(&self, user_id: &str, is_active: bool) -> Result<(), UserStoreError> {
        let result = sqlx::query("UPDATE users SET is_active = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(user_id)
            .bind(i64::from(is_active))
            .bind(now_millis())
            .execute(&self.pool)
            .await
            .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn set_password_hash(
        &self,
        user_id: &str,
        password_hash: &str,
    ) -> Result<(), UserStoreError> {
        let result =
            sqlx::query("UPDATE users SET password_hash = ?2, updated_at = ?3 WHERE id = ?1")
                .bind(user_id)
                .bind(password_hash)
                .bind(now_millis())
                .execute(&self.pool)
                .await
                .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn count(&self) -> Result<u64, UserStoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
            .fetch_one(&self.pool)
            .await
            .map_err(infrastructure)?;
        Ok(count.max(0) as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::password::PasswordHasher;

    /// Exercises the full SQLite implementation against a real in-memory
    /// database, applying the same DDL as the committed migration.
    async fn store() -> SqliteUserStore {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                organization_id TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                is_admin INTEGER NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("CREATE UNIQUE INDEX idx_users_username_lower ON users (LOWER(username))")
            .execute(&pool)
            .await
            .unwrap();
        SqliteUserStore::new(pool)
    }

    fn new_user(username: &str, hash: &str, is_admin: bool) -> NewUser {
        NewUser {
            user_id: uuid::Uuid::new_v4().to_string(),
            username: username.to_string(),
            organization_id: "11111111-1111-1111-1111-111111111111".to_string(),
            password_hash: hash.to_string(),
            is_admin,
        }
    }

    #[tokio::test]
    async fn create_then_find_roundtrips_and_verifies() {
        let store = store().await;
        let hasher = PasswordHasher::new();
        let hash = hasher.hash("correct horse battery").unwrap();
        let created = store.create(new_user("Operator", &hash, true)).await.unwrap();
        assert_eq!(created.username, "operator", "username must be lowercased");

        let found = store.find_by_username("OPERATOR").await.unwrap().unwrap();
        assert_eq!(found.user_id, created.user_id);
        assert!(found.is_admin);
        assert!(found.is_active);
        assert!(hasher.verify("correct horse battery", &found.password_hash).unwrap());
    }

    #[tokio::test]
    async fn duplicate_username_is_reported_distinctly() {
        let store = store().await;
        store.create(new_user("operator", "h", false)).await.unwrap();
        // Different casing must still collide, per the LOWER() unique index.
        let error = store.create(new_user("OperatoR", "h", false)).await.unwrap_err();
        assert_eq!(error, UserStoreError::DuplicateUsername);
    }

    #[tokio::test]
    async fn unknown_username_is_none_not_error() {
        assert!(store().await.find_by_username("nobody").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn count_tracks_inserts_for_bootstrap_check() {
        let store = store().await;
        assert_eq!(store.count().await.unwrap(), 0);
        store.create(new_user("a", "h", true)).await.unwrap();
        store.create(new_user("b", "h", false)).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn set_active_and_password_hash_apply_and_detect_missing() {
        let store = store().await;
        let user = store.create(new_user("operator", "h", false)).await.unwrap();

        store.set_active(&user.user_id, false).await.unwrap();
        assert!(!store.find_by_id(&user.user_id).await.unwrap().unwrap().is_active);

        store.set_password_hash(&user.user_id, "new-hash").await.unwrap();
        let reloaded = store.find_by_id(&user.user_id).await.unwrap().unwrap();
        assert_eq!(reloaded.password_hash, "new-hash");

        let missing = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            store.set_active(&missing, true).await.unwrap_err(),
            UserStoreError::NotFound
        );
        assert_eq!(
            store.set_password_hash(&missing, "x").await.unwrap_err(),
            UserStoreError::NotFound
        );
    }

    #[tokio::test]
    async fn list_returns_all_users_sorted() {
        let store = store().await;
        store.create(new_user("zulu", "h", false)).await.unwrap();
        store.create(new_user("alpha", "h", true)).await.unwrap();
        let names: Vec<_> = store.list().await.unwrap().into_iter().map(|u| u.username).collect();
        assert_eq!(names, vec!["alpha", "zulu"]);
    }
}
