//! Migration runner used by the ONYX release pipeline.
//!
//! The runner deliberately executes `up` twice. SQLx migration tracking makes
//! the second pass a no-op and turns Team 8 ruling R10 into an executable gate.

use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{bail, Context};
use chrono::Utc;
use sqlx::{
    migrate::Migrator,
    postgres::{PgPool, PgPoolOptions},
    sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions},
    Row,
};

static POSTGRES_MIGRATOR: Migrator = sqlx::migrate!("../../../migrations/postgres");
static SQLITE_MIGRATOR: Migrator = sqlx::migrate!("../../../migrations/sqlite");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseKind {
    Postgres,
    Sqlite,
}

impl FromStr for DatabaseKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "postgres" | "postgresql" | "pg" => Ok(Self::Postgres),
            "sqlite" | "sqlite3" => Ok(Self::Sqlite),
            other => bail!("unsupported database kind '{other}'; expected postgres or sqlite"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MigrationStatus {
    pub version: i64,
    pub description: String,
    pub installed: bool,
    pub success: bool,
    pub installed_on: Option<String>,
}

pub async fn migrate_up(kind: DatabaseKind, database_url: &str) -> anyhow::Result<()> {
    match kind {
        DatabaseKind::Postgres => {
            let pool = postgres_pool(database_url).await?;
            POSTGRES_MIGRATOR
                .run(&pool)
                .await
                .context("failed to apply PostgreSQL migrations")?;
            POSTGRES_MIGRATOR
                .run(&pool)
                .await
                .context("PostgreSQL migrations were not idempotent")?;
        }
        DatabaseKind::Sqlite => {
            let pool = sqlite_pool(database_url).await?;
            SQLITE_MIGRATOR
                .run(&pool)
                .await
                .context("failed to apply SQLite migrations")?;
            SQLITE_MIGRATOR
                .run(&pool)
                .await
                .context("SQLite migrations were not idempotent")?;
        }
    }
    Ok(())
}

pub async fn migrate_down(
    kind: DatabaseKind,
    database_url: &str,
    target: i64,
) -> anyhow::Result<()> {
    if target < 0 {
        bail!("migration target must be zero or a positive version");
    }
    match kind {
        DatabaseKind::Postgres => {
            let pool = postgres_pool(database_url).await?;
            POSTGRES_MIGRATOR
                .undo(&pool, target)
                .await
                .with_context(|| format!("failed to roll PostgreSQL back to {target}"))?;
        }
        DatabaseKind::Sqlite => {
            let pool = sqlite_pool(database_url).await?;
            SQLITE_MIGRATOR
                .undo(&pool, target)
                .await
                .with_context(|| format!("failed to roll SQLite back to {target}"))?;
        }
    }
    Ok(())
}

pub async fn migration_status(
    kind: DatabaseKind,
    database_url: &str,
) -> anyhow::Result<Vec<MigrationStatus>> {
    match kind {
        DatabaseKind::Postgres => postgres_status(&postgres_pool(database_url).await?).await,
        DatabaseKind::Sqlite => sqlite_status(&sqlite_pool(database_url).await?).await,
    }
}

pub fn create_migration(root: &Path, name: &str) -> anyhow::Result<Vec<PathBuf>> {
    let normalized = normalize_name(name)?;
    let version = Utc::now().format("%Y%m%d%H%M%S").to_string();
    let mut created = Vec::new();
    for database in ["postgres", "sqlite"] {
        let directory = root.join("migrations").join(database);
        std::fs::create_dir_all(&directory)
            .with_context(|| format!("failed to create {}", directory.display()))?;
        for direction in ["up", "down"] {
            let path = directory.join(format!("{version}_{normalized}.{direction}.sql"));
            let body = format!(
                "-- ONYX migration {version}_{normalized} ({database}, {direction})\n-- Add reversible SQL below.\n"
            );
            std::fs::write(&path, body)
                .with_context(|| format!("failed to create {}", path.display()))?;
            created.push(path);
        }
    }
    Ok(created)
}

async fn postgres_pool(database_url: &str) -> anyhow::Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(2)
        .connect(database_url)
        .await
        .context("failed to connect to PostgreSQL")
}

async fn sqlite_pool(database_url: &str) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::from_str(database_url)
        .context("invalid SQLite URL")?
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .context("failed to connect to SQLite")
}

async fn postgres_status(pool: &PgPool) -> anyhow::Result<Vec<MigrationStatus>> {
    let rows = sqlx::query(
        "SELECT version, description, success, installed_on::text AS installed_on FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut applied = std::collections::BTreeMap::new();
    for row in rows {
        let version: i64 = row.try_get("version")?;
        let description: String = row.try_get("description")?;
        let success: bool = row.try_get("success")?;
        let installed_on: Option<String> = row.try_get("installed_on").ok();
        applied.insert(version, (description, success, installed_on));
    }
    Ok(POSTGRES_MIGRATOR
        .iter()
        .map(|migration| {
            status_from_applied(migration.version, migration.description.as_ref(), &applied)
        })
        .collect())
}

async fn sqlite_status(pool: &SqlitePool) -> anyhow::Result<Vec<MigrationStatus>> {
    let rows = sqlx::query(
        "SELECT version, description, success, CAST(installed_on AS TEXT) AS installed_on FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let mut applied = std::collections::BTreeMap::new();
    for row in rows {
        let version: i64 = row.try_get("version")?;
        let description: String = row.try_get("description")?;
        let success: bool = row.try_get("success")?;
        let installed_on: Option<String> = row.try_get("installed_on").ok();
        applied.insert(version, (description, success, installed_on));
    }
    Ok(SQLITE_MIGRATOR
        .iter()
        .map(|migration| {
            status_from_applied(migration.version, migration.description.as_ref(), &applied)
        })
        .collect())
}

fn status_from_applied(
    version: i64,
    description: &str,
    applied: &std::collections::BTreeMap<i64, (String, bool, Option<String>)>,
) -> MigrationStatus {
    let applied_entry = applied.get(&version);
    MigrationStatus {
        version,
        description: description.to_string(),
        installed: applied_entry.is_some(),
        success: applied_entry.map(|entry| entry.1).unwrap_or(false),
        installed_on: applied_entry.and_then(|entry| entry.2.clone()),
    }
}

fn normalize_name(name: &str) -> anyhow::Result<String> {
    let normalized = name
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                char::from_u32(95).expect("ASCII underscore is valid")
            }
        })
        .collect::<String>()
        .split("_")
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if normalized.is_empty() {
        bail!("migration name must contain at least one alphanumeric character");
    }
    Ok(normalized)
}
