use std::time::Duration;

use sqlx::{PgPool, Row, SqlitePool};

pub const SNAPSHOT_INTERVAL: Duration = Duration::from_secs(60 * 60);
pub const SNAPSHOT_EVENT_THRESHOLD: i64 = 1_000;

pub async fn run_snapshotter(pool: PgPool) -> anyhow::Result<()> {
    loop {
        let created = snapshot_tick_postgres(&pool).await?;
        tracing::info!(created, "aggregate snapshot pass complete");
        tokio::time::sleep(SNAPSHOT_INTERVAL).await;
    }
}

pub async fn snapshot_tick_postgres(pool: &PgPool) -> anyhow::Result<u64> {
    let rows = sqlx::query(
        "SELECT a.id::text AS aggregate_id, COUNT(e.event_id)::bigint AS event_count, COALESCE((SELECT MAX(s.event_count) FROM aggregate_snapshots s WHERE s.aggregate_id = a.id), 0)::bigint AS snapshotted_count FROM aggregates a JOIN domain_events e ON e.aggregate_id = a.id GROUP BY a.id HAVING COUNT(e.event_id) - COALESCE((SELECT MAX(s.event_count) FROM aggregate_snapshots s WHERE s.aggregate_id = a.id), 0) > $1",
    )
    .bind(SNAPSHOT_EVENT_THRESHOLD)
    .fetch_all(pool)
    .await?;
    let mut created = 0_u64;
    for row in rows {
        let aggregate_id: String = row.try_get("aggregate_id")?;
        snapshot_aggregate(pool, &aggregate_id).await?;
        created = created.saturating_add(1);
    }
    Ok(created)
}

pub async fn snapshot_aggregate(pool: &PgPool, aggregate_id: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO aggregate_snapshots (aggregate_id, organization_id, aggregate_type, aggregate_version, event_count, state, created_at_ms) SELECT a.id, a.organization_id, a.aggregate_type, a.version, (SELECT COUNT(*) FROM domain_events e WHERE e.aggregate_id = a.id), a.state, (EXTRACT(EPOCH FROM NOW()) * 1000)::bigint FROM aggregates a WHERE a.id = $1::uuid ON CONFLICT (aggregate_id, aggregate_version) DO NOTHING",
    )
    .bind(aggregate_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn snapshot_tick_sqlite(pool: &SqlitePool) -> anyhow::Result<u64> {
    let rows = sqlx::query(
        "SELECT a.id AS aggregate_id, COUNT(e.event_id) AS event_count, COALESCE((SELECT MAX(s.event_count) FROM aggregate_snapshots s WHERE s.aggregate_id = a.id), 0) AS snapshotted_count FROM aggregates a JOIN domain_events e ON e.aggregate_id = a.id GROUP BY a.id HAVING COUNT(e.event_id) - COALESCE((SELECT MAX(s.event_count) FROM aggregate_snapshots s WHERE s.aggregate_id = a.id), 0) > ?",
    )
    .bind(SNAPSHOT_EVENT_THRESHOLD)
    .fetch_all(pool)
    .await?;
    let mut created = 0_u64;
    for row in rows {
        let aggregate_id: Vec<u8> = row.try_get("aggregate_id")?;
        sqlx::query(
            "INSERT OR IGNORE INTO aggregate_snapshots (aggregate_id, organization_id, aggregate_type, aggregate_version, event_count, state, created_at_ms) SELECT a.id, a.organization_id, a.aggregate_type, a.version, (SELECT COUNT(*) FROM domain_events e WHERE e.aggregate_id = a.id), a.state, CAST(strftime('%s','now') AS INTEGER) * 1000 FROM aggregates a WHERE a.id = ?",
        )
        .bind(aggregate_id)
        .execute(pool)
        .await?;
        created = created.saturating_add(1);
    }
    Ok(created)
}
