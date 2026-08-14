//! Query handlers for aggregate reads and Team 6 projection queries.

use std::{collections::HashMap, sync::Arc};

use platform_kernel::ObjectId;
use query_application::{Loaded, Repository, RepositoryError};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{PgPool, Row, SqlitePool};

/// Database pool used by the web projection query surface.
///
/// SQLite is retained for deterministic local and Team 6 tests. Production
/// composition uses PostgreSQL so horizontally-scaled API replicas read the
/// same authoritative projection store.
#[derive(Clone)]
pub enum ProjectionPool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

/// Load an aggregate by its ID. Retained for existing native-client consumers.
pub async fn load_aggregate(
    id: &ObjectId,
    repo: Arc<dyn Repository>,
) -> Result<Option<Loaded>, RepositoryError> {
    repo.load(id).await
}

#[derive(Debug, Serialize)]
pub struct ProjectionFreshness {
    pub projection_version: u64,
    pub last_updated_at: String,
    pub is_stale: bool,
}

#[derive(Debug)]
pub struct QueryExecution {
    pub data: Vec<Value>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub total_count: usize,
    pub freshness: ProjectionFreshness,
}

#[derive(Debug)]
struct ProjectionRow {
    aggregate_type: String,
    state: Value,
    version: i64,
    updated_at_ms: i64,
}

/// Execute the frozen Team 6 read-model queries against the authoritative
/// aggregate snapshot table. The web client remains projection-only: this
/// function performs no domain mutation and contains no state-machine logic.
pub async fn execute_query(
    pool: &ProjectionPool,
    query_type: &str,
    organization_id: &str,
    filters: &HashMap<String, Value>,
    limit: Option<u32>,
    cursor: Option<&str>,
) -> Result<QueryExecution, sqlx::Error> {
    let organization_id = uuid::Uuid::parse_str(organization_id)
        .map_err(|error| sqlx::Error::Protocol(format!("invalid organization UUID: {error}")))?;
    let max_limit = limit.unwrap_or(100).clamp(1, 200) as usize;
    let offset = cursor
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    if query_type == "dashboard.summary" {
        return dashboard_summary(pool, organization_id).await;
    }

    let aggregate_type = match query_type {
        "mission.list" | "mission.detail" => "mission",
        "task.list" | "task.detail" => "task",
        "timeline.list" => "timeline",
        "notification.list" => "notification",
        "approval.list" => "approval",
        "report.detail" => "report",
        // Added 2026-08-14 for the Admin platform — see
        // routes::command's matching comment.
        "policy.list" | "policy.detail" => "policy",
        "legal_hold.list" | "legal_hold.detail" => "legal_hold",
        _ => return Ok(empty_result()),
    };

    let rows = fetch_rows(pool, organization_id, Some(aggregate_type)).await?;
    let mut projection_version = 0_u64;
    let mut latest_updated = 0_i64;
    let mut values = Vec::new();

    for row in rows {
        projection_version = projection_version.max(row.version.max(0) as u64);
        latest_updated = latest_updated.max(row.updated_at_ms);
        let mut value = row.state;
        normalize_public_state(&mut value);
        if matches_filters(&value, filters) {
            values.push(value);
        }
    }

    let is_detail = matches!(
        query_type,
        "mission.detail" | "task.detail" | "report.detail"
    );
    if is_detail {
        values.truncate(1);
    }
    let total_count = values.len();
    let page: Vec<Value> = values.into_iter().skip(offset).take(max_limit).collect();
    let next_offset = offset + page.len();
    let has_more = !is_detail && next_offset < total_count;

    Ok(QueryExecution {
        data: page,
        has_more,
        next_cursor: has_more.then(|| next_offset.to_string()),
        total_count,
        freshness: ProjectionFreshness {
            projection_version,
            last_updated_at: millis_to_iso(latest_updated),
            is_stale: false,
        },
    })
}

async fn fetch_rows(
    pool: &ProjectionPool,
    organization_id: uuid::Uuid,
    aggregate_type: Option<&str>,
) -> Result<Vec<ProjectionRow>, sqlx::Error> {
    match pool {
        ProjectionPool::Sqlite(pool) => {
            let rows = if let Some(aggregate_type) = aggregate_type {
                sqlx::query(
                    "SELECT aggregate_type, state, version, updated_at AS updated_at_ms \
                     FROM aggregates WHERE organization_id = ? AND aggregate_type = ? \
                     ORDER BY updated_at DESC",
                )
                .bind(organization_id.as_bytes().to_vec())
                .bind(aggregate_type)
                .fetch_all(pool)
                .await?
            } else {
                sqlx::query(
                    "SELECT aggregate_type, state, version, updated_at AS updated_at_ms \
                     FROM aggregates WHERE organization_id = ? ORDER BY updated_at DESC",
                )
                .bind(organization_id.as_bytes().to_vec())
                .fetch_all(pool)
                .await?
            };

            rows.into_iter()
                .map(|row| {
                    let state_text: String = row.try_get("state")?;
                    Ok(ProjectionRow {
                        aggregate_type: row.try_get("aggregate_type")?,
                        state: serde_json::from_str(&state_text).unwrap_or_else(|_| json!({})),
                        version: row.try_get("version")?,
                        updated_at_ms: row.try_get("updated_at_ms")?,
                    })
                })
                .collect()
        }
        ProjectionPool::Postgres(pool) => {
            let rows = if let Some(aggregate_type) = aggregate_type {
                sqlx::query(
                    "SELECT aggregate_type, state, version, \
                     (EXTRACT(EPOCH FROM updated_at) * 1000)::BIGINT AS updated_at_ms \
                     FROM aggregates WHERE organization_id = $1 AND aggregate_type = $2 \
                     ORDER BY updated_at DESC",
                )
                .bind(organization_id)
                .bind(aggregate_type)
                .fetch_all(pool)
                .await?
            } else {
                sqlx::query(
                    "SELECT aggregate_type, state, version, \
                     (EXTRACT(EPOCH FROM updated_at) * 1000)::BIGINT AS updated_at_ms \
                     FROM aggregates WHERE organization_id = $1 ORDER BY updated_at DESC",
                )
                .bind(organization_id)
                .fetch_all(pool)
                .await?
            };

            rows.into_iter()
                .map(|row| {
                    Ok(ProjectionRow {
                        aggregate_type: row.try_get("aggregate_type")?,
                        state: row.try_get("state")?,
                        version: row.try_get("version")?,
                        updated_at_ms: row.try_get("updated_at_ms")?,
                    })
                })
                .collect()
        }
    }
}

fn normalize_public_state(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let public_id = object
        .get("public_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    object.remove("public_id");
    object.remove("id");
    if let Some(id) = public_id {
        object.insert("id".to_string(), Value::String(id));
    }
}

fn matches_filters(value: &Value, filters: &HashMap<String, Value>) -> bool {
    filters.iter().all(|(key, expected)| {
        if expected.is_null() {
            return true;
        }
        match value.get(key) {
            Some(actual) if expected.is_array() => expected
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item == actual)),
            Some(actual) => actual == expected,
            None => false,
        }
    })
}

async fn dashboard_summary(
    pool: &ProjectionPool,
    organization_id: uuid::Uuid,
) -> Result<QueryExecution, sqlx::Error> {
    let rows = fetch_rows(pool, organization_id, None).await?;
    let mut missions = 0_u64;
    let mut active_missions = 0_u64;
    let mut tasks = 0_u64;
    let mut blocked_tasks = 0_u64;
    let mut unread_notifications = 0_u64;
    let mut pending_approvals = 0_u64;
    let mut projection_version = 0_u64;
    let mut latest_updated = 0_i64;
    let mut activity = Vec::new();

    for row in rows {
        projection_version = projection_version.max(row.version.max(0) as u64);
        latest_updated = latest_updated.max(row.updated_at_ms);
        let mut state = row.state;
        normalize_public_state(&mut state);
        let status = state
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match row.aggregate_type.as_str() {
            "mission" => {
                missions += 1;
                if status == "active" {
                    active_missions += 1;
                }
            }
            "task" => {
                tasks += 1;
                if status == "blocked" {
                    blocked_tasks += 1;
                }
            }
            "notification" => {
                if status == "unacknowledged" {
                    unread_notifications += 1;
                }
            }
            "approval" if status == "pending" => {
                pending_approvals += 1;
            }
            "approval" => {}
            _ => {}
        }
        if matches!(
            row.aggregate_type.as_str(),
            "notification" | "approval" | "mission" | "task"
        ) {
            activity.push(json!({
                "id": state.get("id").cloned().unwrap_or(Value::Null),
                "type": row.aggregate_type,
                "title": state
                    .get("title")
                    .or_else(|| state.get("name"))
                    .cloned()
                    .unwrap_or(Value::String("Activity".into())),
                "status": status,
                "updated_at": state
                    .get("updated_at")
                    .or_else(|| state.get("created_at"))
                    .cloned()
                    .unwrap_or(Value::String(millis_to_iso(row.updated_at_ms)))
            }));
        }
    }
    activity.truncate(8);

    Ok(QueryExecution {
        data: vec![json!({
            "missions": missions,
            "active_missions": active_missions,
            "tasks": tasks,
            "blocked_tasks": blocked_tasks,
            "unread_notifications": unread_notifications,
            "pending_approvals": pending_approvals,
            "activity": activity
        })],
        has_more: false,
        next_cursor: None,
        total_count: 1,
        freshness: ProjectionFreshness {
            projection_version,
            last_updated_at: millis_to_iso(latest_updated),
            is_stale: false,
        },
    })
}

fn empty_result() -> QueryExecution {
    QueryExecution {
        data: vec![],
        has_more: false,
        next_cursor: None,
        total_count: 0,
        freshness: ProjectionFreshness {
            projection_version: 0,
            last_updated_at: millis_to_iso(0),
            is_stale: false,
        },
    }
}

fn millis_to_iso(millis: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(millis)
        .map(|datetime| datetime.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
}
