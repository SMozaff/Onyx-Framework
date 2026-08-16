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
use security_application::{NewUser, UserClass, UserRecord, UserStore, UserStoreError};
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

    /// Whether `candidate` appears in `start`'s ancestor chain (i.e.
    /// walking `parent_user_id` repeatedly from `start` eventually
    /// reaches `candidate`). Used by `set_parent` to reject an
    /// assignment that would create a cycle — see that method's doc
    /// comment. Bounded by `MAX_DEPTH` as a defensive measure against an
    /// already-corrupt cycle in the stored data causing an infinite
    /// loop here; a real organizational hierarchy is not going to be
    /// anywhere near this deep, so hitting the bound itself is treated
    /// as "yes, treat this as a cycle" (fail closed) rather than
    /// silently returning false.
    async fn is_ancestor(
        &self,
        candidate: uuid::Uuid,
        start: uuid::Uuid,
    ) -> Result<bool, UserStoreError> {
        const MAX_DEPTH: u32 = 1000;
        let mut current = start;
        for _ in 0..MAX_DEPTH {
            if current == candidate {
                return Ok(true);
            }
            let next: Option<uuid::Uuid> =
                sqlx::query_scalar("SELECT parent_user_id FROM users WHERE id = $1")
                    .bind(current)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(infrastructure)?
                    .flatten();
            match next {
                Some(parent) => current = parent,
                None => return Ok(false),
            }
        }
        Ok(true)
    }
}

/// Like `insert_error`, but also maps a foreign-key violation on
/// `fk_column` to [`UserStoreError::ParentNotFound`] — used by writes
/// that set `parent_user_id`, where a stale/invalid id should surface
/// as that specific error rather than a generic infrastructure failure.
fn insert_error_with_fk(error: sqlx::Error, fk_column: &str) -> UserStoreError {
    if let sqlx::Error::Database(db) = &error {
        // Postgres reports a foreign-key violation as SQLSTATE 23503.
        if db.code().as_deref() == Some("23503") {
            return UserStoreError::ParentNotFound;
        }
        let message = db.message().to_ascii_lowercase();
        if message.contains(&fk_column.to_ascii_lowercase()) && message.contains("constraint") {
            return UserStoreError::ParentNotFound;
        }
    }
    insert_error(error)
}

fn record_from_pg_row(row: &sqlx::postgres::PgRow) -> Result<UserRecord, UserStoreError> {
    let id: uuid::Uuid = row.try_get("id").map_err(infrastructure)?;
    let organization_id: uuid::Uuid = row.try_get("organization_id").map_err(infrastructure)?;
    let class_raw: Option<String> = row.try_get("class").map_err(infrastructure)?;
    let class = parse_class(class_raw)?;
    let parent_user_id: Option<uuid::Uuid> =
        row.try_get("parent_user_id").map_err(infrastructure)?;
    Ok(UserRecord {
        user_id: id.to_string(),
        username: row.try_get("username").map_err(infrastructure)?,
        organization_id: organization_id.to_string(),
        password_hash: row.try_get("password_hash").map_err(infrastructure)?,
        is_admin: row.try_get("is_admin").map_err(infrastructure)?,
        is_manager: row.try_get("is_manager").map_err(infrastructure)?,
        class,
        parent_user_id: parent_user_id.map(|id| id.to_string()),
        is_active: row.try_get("is_active").map_err(infrastructure)?,
    })
}

/// Parses a stored `class` value, treating an unrecognized non-NULL
/// string as an infrastructure error rather than silently mapping it to
/// `None` — a value that fails to parse indicates the stored data and
/// this code have drifted (e.g. a manual DB edit, or a future enum
/// variant an older binary doesn't know about yet), which should be
/// surfaced, not swallowed into "unclassified."
fn parse_class(raw: Option<String>) -> Result<Option<UserClass>, UserStoreError> {
    match raw {
        None => Ok(None),
        Some(s) => UserClass::parse(&s).map(Some).ok_or_else(|| {
            UserStoreError::Infrastructure(format!("unrecognized stored class value: {s}"))
        }),
    }
}

fn parse_uuid(value: &str, field: &str) -> Result<uuid::Uuid, UserStoreError> {
    uuid::Uuid::parse_str(value)
        .map_err(|_| UserStoreError::Infrastructure(format!("{field} is not a valid UUID")))
}

#[async_trait]
impl UserStore for PostgresUserStore {
    async fn find_by_username(&self, username: &str) -> Result<Option<UserRecord>, UserStoreError> {
        let row = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_manager, \
             class, parent_user_id, is_active \
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
            "SELECT id, username, organization_id, password_hash, is_admin, is_manager, \
             class, parent_user_id, is_active \
             FROM users WHERE id = $1",
        )
        .bind(parse_uuid(user_id, "user_id")?)
        .fetch_optional(&self.pool)
        .await
        .map_err(infrastructure)?;
        row.as_ref().map(record_from_pg_row).transpose()
    }

    async fn create(&self, user: NewUser) -> Result<UserRecord, UserStoreError> {
        let parent_uuid = user
            .parent_user_id
            .as_deref()
            .map(|p| parse_uuid(p, "parent_user_id"))
            .transpose()?;
        sqlx::query(
            "INSERT INTO users (id, username, organization_id, password_hash, is_admin, \
             is_manager, class, parent_user_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(parse_uuid(&user.user_id, "user_id")?)
        .bind(user.username.to_lowercase())
        .bind(parse_uuid(&user.organization_id, "organization_id")?)
        .bind(&user.password_hash)
        .bind(user.is_admin)
        .bind(user.is_manager)
        .bind(user.class.map(|c| c.as_str()))
        .bind(parent_uuid)
        .execute(&self.pool)
        .await
        .map_err(|e| insert_error_with_fk(e, "parent_user_id"))?;
        Ok(UserRecord {
            user_id: user.user_id,
            username: user.username.to_lowercase(),
            organization_id: user.organization_id,
            password_hash: user.password_hash,
            is_admin: user.is_admin,
            is_manager: user.is_manager,
            class: user.class,
            parent_user_id: user.parent_user_id,
            is_active: true,
        })
    }

    async fn list(&self) -> Result<Vec<UserRecord>, UserStoreError> {
        let rows = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_manager, \
             class, parent_user_id, is_active \
             FROM users ORDER BY username",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(infrastructure)?;
        rows.iter().map(record_from_pg_row).collect()
    }

    async fn set_active(&self, user_id: &str, is_active: bool) -> Result<(), UserStoreError> {
        let result =
            sqlx::query("UPDATE users SET is_active = $2, updated_at = NOW() WHERE id = $1")
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

    async fn set_manager(&self, user_id: &str, is_manager: bool) -> Result<(), UserStoreError> {
        let result =
            sqlx::query("UPDATE users SET is_manager = $2, updated_at = NOW() WHERE id = $1")
                .bind(parse_uuid(user_id, "user_id")?)
                .bind(is_manager)
                .execute(&self.pool)
                .await
                .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn set_class(
        &self,
        user_id: &str,
        class: Option<UserClass>,
    ) -> Result<(), UserStoreError> {
        let result = sqlx::query("UPDATE users SET class = $2, updated_at = NOW() WHERE id = $1")
            .bind(parse_uuid(user_id, "user_id")?)
            .bind(class.map(|c| c.as_str()))
            .execute(&self.pool)
            .await
            .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn set_parent(
        &self,
        user_id: &str,
        parent_user_id: Option<&str>,
    ) -> Result<(), UserStoreError> {
        let target = parse_uuid(user_id, "user_id")?;
        let parent = parent_user_id
            .map(|p| parse_uuid(p, "parent_user_id"))
            .transpose()?;

        if let Some(parent) = parent {
            if parent == target {
                return Err(UserStoreError::ParentCycle);
            }
            // Cycle check beyond the trivial self-reference case: walk
            // the candidate parent's own ancestry and reject if `target`
            // appears in it (which would make `target` its own
            // ancestor once the edge target -> parent is added). No
            // database constraint can express this (see this module's
            // migration notes) — it is checked here, in application
            // code, before the write.
            if self.is_ancestor(target, parent).await? {
                return Err(UserStoreError::ParentCycle);
            }
        }

        let result = sqlx::query(
            "UPDATE users SET parent_user_id = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(target)
        .bind(parent)
        .execute(&self.pool)
        .await
        .map_err(|e| insert_error_with_fk(e, "parent_user_id"))?;
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
            sqlx::query("UPDATE users SET password_hash = $2, updated_at = NOW() WHERE id = $1")
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

    /// Whether `user_id` exists. Used by `create`/`set_parent` to check
    /// `parent_user_id` references a real row — SQLite's FK on this
    /// column is unenforced in this project (no connection sets
    /// `PRAGMA foreign_keys = ON`; see the SQLite migration's own
    /// note), so this check is not optional here the way it might be on
    /// Postgres.
    async fn user_exists(&self, user_id: &str) -> Result<bool, UserStoreError> {
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
            .map_err(infrastructure)?;
        Ok(count > 0)
    }

    /// Whether `candidate` appears in `start`'s ancestor chain. See
    /// `PostgresUserStore::is_ancestor`'s doc comment for the full
    /// rationale — identical logic, SQLite query syntax.
    async fn is_ancestor(&self, candidate: &str, start: &str) -> Result<bool, UserStoreError> {
        const MAX_DEPTH: u32 = 1000;
        let mut current = start.to_string();
        for _ in 0..MAX_DEPTH {
            if current == candidate {
                return Ok(true);
            }
            let next: Option<String> =
                sqlx::query_scalar("SELECT parent_user_id FROM users WHERE id = ?1")
                    .bind(&current)
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(infrastructure)?
                    .flatten();
            match next {
                Some(parent) => current = parent,
                None => return Ok(false),
            }
        }
        Ok(true)
    }
}

fn record_from_sqlite_row(row: &sqlx::sqlite::SqliteRow) -> Result<UserRecord, UserStoreError> {
    // SQLite stores the booleans as INTEGER (see the migration), so they are
    // read as i64 and normalised here rather than relying on driver coercion.
    let is_admin: i64 = row.try_get("is_admin").map_err(infrastructure)?;
    let is_manager: i64 = row.try_get("is_manager").map_err(infrastructure)?;
    let is_active: i64 = row.try_get("is_active").map_err(infrastructure)?;
    let class_raw: Option<String> = row.try_get("class").map_err(infrastructure)?;
    let class = parse_class(class_raw)?;
    let parent_user_id: Option<String> =
        row.try_get("parent_user_id").map_err(infrastructure)?;
    Ok(UserRecord {
        user_id: row.try_get("id").map_err(infrastructure)?,
        username: row.try_get("username").map_err(infrastructure)?,
        organization_id: row.try_get("organization_id").map_err(infrastructure)?,
        password_hash: row.try_get("password_hash").map_err(infrastructure)?,
        is_admin: is_admin != 0,
        is_manager: is_manager != 0,
        class,
        parent_user_id,
        is_active: is_active != 0,
    })
}

#[async_trait]
impl UserStore for SqliteUserStore {
    async fn find_by_username(&self, username: &str) -> Result<Option<UserRecord>, UserStoreError> {
        let row = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_manager, \
             class, parent_user_id, is_active \
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
            "SELECT id, username, organization_id, password_hash, is_admin, is_manager, \
             class, parent_user_id, is_active \
             FROM users WHERE id = ?1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(infrastructure)?;
        row.as_ref().map(record_from_sqlite_row).transpose()
    }

    async fn create(&self, user: NewUser) -> Result<UserRecord, UserStoreError> {
        if let Some(parent) = user.parent_user_id.as_deref() {
            if !self.user_exists(parent).await? {
                return Err(UserStoreError::ParentNotFound);
            }
        }
        let now = now_millis();
        sqlx::query(
            "INSERT INTO users (id, username, organization_id, password_hash, is_admin, \
             is_manager, class, parent_user_id, is_active, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)",
        )
        .bind(&user.user_id)
        .bind(user.username.to_lowercase())
        .bind(&user.organization_id)
        .bind(&user.password_hash)
        .bind(i64::from(user.is_admin))
        .bind(i64::from(user.is_manager))
        .bind(user.class.map(|c| c.as_str()))
        .bind(&user.parent_user_id)
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
            is_manager: user.is_manager,
            class: user.class,
            parent_user_id: user.parent_user_id,
            is_active: true,
        })
    }

    async fn list(&self) -> Result<Vec<UserRecord>, UserStoreError> {
        let rows = sqlx::query(
            "SELECT id, username, organization_id, password_hash, is_admin, is_manager, \
             class, parent_user_id, is_active \
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

    async fn set_manager(&self, user_id: &str, is_manager: bool) -> Result<(), UserStoreError> {
        let result =
            sqlx::query("UPDATE users SET is_manager = ?2, updated_at = ?3 WHERE id = ?1")
                .bind(user_id)
                .bind(i64::from(is_manager))
                .bind(now_millis())
                .execute(&self.pool)
                .await
                .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn set_class(
        &self,
        user_id: &str,
        class: Option<UserClass>,
    ) -> Result<(), UserStoreError> {
        let result = sqlx::query("UPDATE users SET class = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(user_id)
            .bind(class.map(|c| c.as_str()))
            .bind(now_millis())
            .execute(&self.pool)
            .await
            .map_err(infrastructure)?;
        if result.rows_affected() == 0 {
            return Err(UserStoreError::NotFound);
        }
        Ok(())
    }

    async fn set_parent(
        &self,
        user_id: &str,
        parent_user_id: Option<&str>,
    ) -> Result<(), UserStoreError> {
        if !self.user_exists(user_id).await? {
            return Err(UserStoreError::NotFound);
        }
        if let Some(parent) = parent_user_id {
            if parent == user_id {
                return Err(UserStoreError::ParentCycle);
            }
            if !self.user_exists(parent).await? {
                return Err(UserStoreError::ParentNotFound);
            }
            if self.is_ancestor(user_id, parent).await? {
                return Err(UserStoreError::ParentCycle);
            }
        }

        sqlx::query("UPDATE users SET parent_user_id = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(user_id)
            .bind(parent_user_id)
            .bind(now_millis())
            .execute(&self.pool)
            .await
            .map_err(infrastructure)?;
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
        // Phase A fix (User Hierarchy): foreign_keys(true) here too, so
        // this test suite actually exercises the same enforcement
        // behavior production connections now get — a test fixture
        // silently weaker than production would defeat the point of
        // fixing this at all.
        //
        // A named, shared-cache in-memory database
        // (`file:...?mode=memory&cache=shared`), not a plain
        // `:memory:` filename: `SqliteConnectOptions::new().filename(...)`
        // does not get sqlx's special-cased "sqlite::memory:" URL
        // behavior (a single database shared across every connection
        // the pool opens) — each connection to a plain `:memory:`
        // filename gets its **own**, separate, empty database. With
        // more than one pooled connection (the default), the
        // CREATE TABLE below would land on one connection and every
        // subsequent query might hit a different, empty one — caught
        // by actually running this test suite after the fix, not
        // assumed to work from the `foreign_keys(true)` change alone.
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(format!("file:security_adapter_test_{}?mode=memory&cache=shared", uuid::Uuid::new_v4()))
            .foreign_keys(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        sqlx::query(
            "CREATE TABLE users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                organization_id TEXT NOT NULL,
                password_hash TEXT NOT NULL,
                is_admin INTEGER NOT NULL DEFAULT 0,
                is_manager INTEGER NOT NULL DEFAULT 0,
                class TEXT NULL,
                parent_user_id TEXT NULL REFERENCES users(id),
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
        sqlx::query("CREATE INDEX idx_users_parent_user_id ON users (parent_user_id)")
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
            is_manager: false,
            class: None,
            parent_user_id: None,
        }
    }

    #[tokio::test]
    async fn create_then_find_roundtrips_and_verifies() {
        let store = store().await;
        let hasher = PasswordHasher::new();
        let hash = hasher.hash("correct horse battery").unwrap();
        let created = store
            .create(new_user("Operator", &hash, true))
            .await
            .unwrap();
        assert_eq!(created.username, "operator", "username must be lowercased");

        let found = store.find_by_username("OPERATOR").await.unwrap().unwrap();
        assert_eq!(found.user_id, created.user_id);
        assert!(found.is_admin);
        assert!(found.is_active);
        assert!(hasher
            .verify("correct horse battery", &found.password_hash)
            .unwrap());
    }

    #[tokio::test]
    async fn duplicate_username_is_reported_distinctly() {
        let store = store().await;
        store
            .create(new_user("operator", "h", false))
            .await
            .unwrap();
        // Different casing must still collide, per the LOWER() unique index.
        let error = store
            .create(new_user("OperatoR", "h", false))
            .await
            .unwrap_err();
        assert_eq!(error, UserStoreError::DuplicateUsername);
    }

    #[tokio::test]
    async fn unknown_username_is_none_not_error() {
        assert!(store()
            .await
            .find_by_username("nobody")
            .await
            .unwrap()
            .is_none());
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
        let user = store
            .create(new_user("operator", "h", false))
            .await
            .unwrap();

        store.set_active(&user.user_id, false).await.unwrap();
        assert!(
            !store
                .find_by_id(&user.user_id)
                .await
                .unwrap()
                .unwrap()
                .is_active
        );

        store
            .set_password_hash(&user.user_id, "new-hash")
            .await
            .unwrap();
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
    async fn set_manager_applies_and_detects_missing() {
        let store = store().await;
        let user = store
            .create(new_user("operator", "h", false))
            .await
            .unwrap();
        assert!(!user.is_manager);

        store.set_manager(&user.user_id, true).await.unwrap();
        assert!(
            store
                .find_by_id(&user.user_id)
                .await
                .unwrap()
                .unwrap()
                .is_manager
        );

        // Independent of is_admin — granting Manager does not grant Admin.
        assert!(
            !store
                .find_by_id(&user.user_id)
                .await
                .unwrap()
                .unwrap()
                .is_admin
        );

        let missing = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            store.set_manager(&missing, true).await.unwrap_err(),
            UserStoreError::NotFound
        );
    }

    #[tokio::test]
    async fn set_class_applies_and_detects_missing() {
        let store = store().await;
        let user = store
            .create(new_user("staffer", "h", false))
            .await
            .unwrap();
        assert_eq!(user.class, None);

        store
            .set_class(&user.user_id, Some(UserClass::TeamLeader))
            .await
            .unwrap();
        let reloaded = store.find_by_id(&user.user_id).await.unwrap().unwrap();
        assert_eq!(reloaded.class, Some(UserClass::TeamLeader));

        // Clearing back to None must also work — None is a real,
        // distinct state ("unclassified"), not just the zero value of
        // some other type.
        store.set_class(&user.user_id, None).await.unwrap();
        let cleared = store.find_by_id(&user.user_id).await.unwrap().unwrap();
        assert_eq!(cleared.class, None);

        let missing = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            store
                .set_class(&missing, Some(UserClass::Staff))
                .await
                .unwrap_err(),
            UserStoreError::NotFound
        );
    }

    #[tokio::test]
    async fn class_can_be_set_at_creation_time() {
        // Design doc §5 question 2: class assignment is allowed both at
        // creation and later — this covers the "at creation" half.
        let store = store().await;
        let mut user = new_user("manager1", "h", false);
        user.class = Some(UserClass::TopLevelManager);
        let created = store.create(user).await.unwrap();
        assert_eq!(created.class, Some(UserClass::TopLevelManager));

        let reloaded = store.find_by_id(&created.user_id).await.unwrap().unwrap();
        assert_eq!(reloaded.class, Some(UserClass::TopLevelManager));
    }

    #[tokio::test]
    async fn set_parent_applies_and_detects_missing_user() {
        let store = store().await;
        let manager = store
            .create(new_user("manager1", "h", false))
            .await
            .unwrap();
        let staff = store.create(new_user("staff1", "h", false)).await.unwrap();

        store
            .set_parent(&staff.user_id, Some(&manager.user_id))
            .await
            .unwrap();
        let reloaded = store.find_by_id(&staff.user_id).await.unwrap().unwrap();
        assert_eq!(reloaded.parent_user_id, Some(manager.user_id.clone()));

        let missing = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            store
                .set_parent(&missing, Some(&manager.user_id))
                .await
                .unwrap_err(),
            UserStoreError::NotFound
        );
    }

    #[tokio::test]
    async fn set_parent_rejects_nonexistent_parent() {
        let store = store().await;
        let staff = store.create(new_user("staff1", "h", false)).await.unwrap();
        let bogus_parent = uuid::Uuid::new_v4().to_string();

        assert_eq!(
            store
                .set_parent(&staff.user_id, Some(&bogus_parent))
                .await
                .unwrap_err(),
            UserStoreError::ParentNotFound
        );
    }

    #[tokio::test]
    async fn set_parent_rejects_self_reference() {
        let store = store().await;
        let user = store.create(new_user("solo", "h", false)).await.unwrap();

        assert_eq!(
            store
                .set_parent(&user.user_id, Some(&user.user_id))
                .await
                .unwrap_err(),
            UserStoreError::ParentCycle
        );
    }

    #[tokio::test]
    async fn set_parent_rejects_indirect_cycle() {
        // A -> B -> C already exists (C's parent is B, B's parent is
        // A). Attempting to then set A's parent to C would close the
        // loop (A -> C -> B -> A) — must be rejected even though C is
        // not A's *direct* child, exercising the ancestor-walk, not
        // just the trivial self-reference check.
        let store = store().await;
        let a = store.create(new_user("a", "h", false)).await.unwrap();
        let b = store.create(new_user("b", "h", false)).await.unwrap();
        let c = store.create(new_user("c", "h", false)).await.unwrap();

        store.set_parent(&b.user_id, Some(&a.user_id)).await.unwrap();
        store.set_parent(&c.user_id, Some(&b.user_id)).await.unwrap();

        assert_eq!(
            store
                .set_parent(&a.user_id, Some(&c.user_id))
                .await
                .unwrap_err(),
            UserStoreError::ParentCycle
        );
    }

    #[tokio::test]
    async fn set_parent_can_clear_to_none() {
        let store = store().await;
        let manager = store
            .create(new_user("manager1", "h", false))
            .await
            .unwrap();
        let staff = store.create(new_user("staff1", "h", false)).await.unwrap();
        store
            .set_parent(&staff.user_id, Some(&manager.user_id))
            .await
            .unwrap();

        store.set_parent(&staff.user_id, None).await.unwrap();
        let reloaded = store.find_by_id(&staff.user_id).await.unwrap().unwrap();
        assert_eq!(reloaded.parent_user_id, None);
    }

    #[tokio::test]
    async fn parent_can_be_set_at_creation_time() {
        let store = store().await;
        let manager = store
            .create(new_user("manager1", "h", false))
            .await
            .unwrap();
        let mut staff_new = new_user("staff1", "h", false);
        staff_new.parent_user_id = Some(manager.user_id.clone());
        let staff = store.create(staff_new).await.unwrap();

        assert_eq!(staff.parent_user_id, Some(manager.user_id));
    }

    #[tokio::test]
    async fn create_rejects_nonexistent_parent() {
        let store = store().await;
        let bogus_parent = uuid::Uuid::new_v4().to_string();
        let mut staff_new = new_user("staff1", "h", false);
        staff_new.parent_user_id = Some(bogus_parent);

        assert_eq!(
            store.create(staff_new).await.unwrap_err(),
            UserStoreError::ParentNotFound
        );
    }

    #[tokio::test]
    async fn list_returns_all_users_sorted() {
        let store = store().await;
        store.create(new_user("zulu", "h", false)).await.unwrap();
        store.create(new_user("alpha", "h", true)).await.unwrap();
        let names: Vec<_> = store
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|u| u.username)
            .collect();
        assert_eq!(names, vec!["alpha", "zulu"]);
    }
}
