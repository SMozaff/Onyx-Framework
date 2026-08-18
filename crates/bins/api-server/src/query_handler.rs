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
pub(crate) struct ProjectionRow {
    pub(crate) aggregate_type: String,
    pub(crate) state: Value,
    pub(crate) version: i64,
    pub(crate) updated_at_ms: i64,
}

/// Execute the frozen Team 6 read-model queries against the authoritative
/// aggregate snapshot table. The web client remains projection-only: this
/// function performs no domain mutation and contains no state-machine logic.
/// # `viewer` (added 2026-08-16, D.5)
/// The authenticated caller's user id, when known. Only used to redact
/// `todo_list.*`/`target_list.*` results — see
/// `redact_team_leader_pre_check_for_viewer`'s doc comment for the
/// exact rule. `None` preserves this function's prior behavior exactly
/// (no redaction applied) — every other query type (mission, task,
/// policy, etc.) is unaffected regardless of `viewer`, since redaction
/// only triggers for the two aggregate types that carry a Team Leader
/// pre-check.
pub async fn execute_query(
    pool: &ProjectionPool,
    query_type: &str,
    organization_id: &str,
    filters: &HashMap<String, Value>,
    limit: Option<u32>,
    cursor: Option<&str>,
    viewer: Option<ObjectId>,
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
        // TodoList/TargetList/StaffLoan (added 2026-08-16 — see
        // DESIGN_User_Hierarchy_Chain_of_Authority.md §4, §2.1).
        "todo_list.list" | "todo_list.detail" => "todo_list",
        "target_list.list" | "target_list.detail" => "target_list",
        "staff_loan.list" | "staff_loan.detail" => "staff_loan",
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
        if aggregate_type == "todo_list" || aggregate_type == "target_list" {
            redact_team_leader_pre_check_for_viewer(&mut value, viewer);
        }
        if matches_filters(&value, filters) {
            values.push(value);
        }
    }

    let is_detail = matches!(
        query_type,
        "mission.detail"
            | "task.detail"
            | "report.detail"
            | "policy.detail"
            | "legal_hold.detail"
            | "todo_list.detail"
            | "target_list.detail"
            | "staff_loan.detail"
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

pub(crate) async fn fetch_rows(
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

/// Normalizes an aggregate's raw `id` field for the wire.
///
/// # The gap this fixes (found 2026-08-14, confirmed pre-existing and
/// affecting every aggregate type, not just Policy/LegalHold)
/// Every domain aggregate's id type (`MissionId`, `PolicyId`, etc.) is a
/// single-field tuple struct wrapping `platform_kernel::ObjectId([u8;
/// 16])`. Serde's default behavior for a newtype is transparent
/// serialization — confirmed empirically, not assumed — so `id` reaches
/// this point as a flat JSON array of 16 numbers, not a string. Every
/// `web-ui` page that keys off `.id` (`Missions`, `Tasks`, `Approvals`,
/// `timeline.list` filtered by `subject_id: mission.id`, etc.) was
/// therefore receiving a byte array where it expected a UUID string.
/// The old `public_id`-remapping logic below never actually applied
/// (`public_id` does not exist on any aggregate — confirmed by search),
/// so this was silently broken for every aggregate type this route
/// serves, not just newly-added ones.
///
/// Converts any 16-element array of small integers (0-255) found at
/// `id` into the equivalent UUID string. Deliberately structural
/// (shape-based), not aggregate-type-specific — this fixes Mission,
/// Task, Notification, Approval, Report, Policy, and LegalHold in one
/// place rather than requiring a per-type case here.
/// Test-only public entry point for `normalize_public_state`, so
/// `tests/query_id_normalization.rs` can exercise the id-shape
/// conversion directly against a synthetic aggregate row shape, without
/// needing a full Policy aggregate creation path (Policy creation is
/// deliberately not exposed as a raw HTTP route yet — see
/// `DECISIONS.md`'s "Admin Platform" section).
#[doc(hidden)]
pub fn normalize_public_state_for_test(value: &mut Value) {
    normalize_public_state(value);
}

fn normalize_public_state(value: &mut Value) {
    let Some(object) = value.as_object_mut() else {
        return;
    };

    // Retained for forward-compatibility, though `public_id` is not
    // actually present on any aggregate today (confirmed by search) --
    // removing this silently would be a second, separate change from
    // the id-array fix below, so it stays as documented dead weight
    // rather than being pulled out in this same pass.
    let public_id = object
        .get("public_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    object.remove("public_id");

    if let Some(id) = public_id {
        object.insert("id".to_string(), Value::String(id));
    } else if let Some(uuid_string) = object.get("id").and_then(object_id_array_to_uuid_string) {
        object.insert("id".to_string(), Value::String(uuid_string));
    }

    // Extended 2026-08-17: normalize every top-level field, not just
    // `id`. A real, live bug was found in this session's own
    // StaffLoansPage UI (`staff_user_id === user.id` comparing a raw
    // byte array to a UUID string, silently never matching) -- the same
    // root cause the doc comment above already describes for `id`
    // ("this was silently broken for every aggregate type... not just
    // newly-added ones"), just not yet fixed for every *field*.
    // ObjectId is always a 16-byte array in this codebase (confirmed:
    // `platform_kernel::ObjectId(pub [u8; 16])`), so any field shaped
    // this way is always an id worth normalizing -- deliberately
    // structural/shape-based, not a per-field-name allowlist, matching
    // this function's existing `id`-handling philosophy. Top-level
    // only (not recursive into nested objects/arrays like
    // `team_leader_pre_check.checked_by` or `items[].item_id`) -- no
    // known nested id field is compared against a UUID string
    // client-side today, so recursing is deferred until a real need
    // appears rather than built speculatively.
    let keys: Vec<String> = object.keys().cloned().collect();
    for key in keys {
        if key == "id" {
            continue; // already handled above
        }
        if let Some(uuid_string) = object.get(&key).and_then(object_id_array_to_uuid_string) {
            object.insert(key, Value::String(uuid_string));
        }
    }
}

/// D.5 (added 2026-08-16): redacts a `TodoList`/`TargetList` result's
/// Team Leader pre-check **substance** (`notes`) when `viewer` is the
/// list's own `owner` — design doc §2.2's visibility rule, resolved
/// 2026-08-15: "Staff must NEVER see the substance/content/quality-
/// judgment of a Team Leader's pre-check — only existence
/// (status/timestamp/who)."
///
/// # Why "viewer == owner" is the redaction rule
/// This function has no access to the viewer's `UserClass` — only
/// their id (see `execute_query`'s `viewer` parameter doc comment for
/// why threading a full class lookup through this path was judged out
/// of scope for this pass). But design doc §2.2's rule is specifically
/// about **Staff** seeing their own pre-check, and a list's `owner` is
/// by construction the Staff member the list is about (design doc
/// §4.0.1's origin field — `ManagerAssigned` or `StaffAuthored` — never
/// changes who `owner` is, only who authored the list). So "is the
/// viewer the owner" is exactly the case design doc §2.2 requires
/// redaction for, without needing a class lookup: nobody else viewing
/// this list is the Staff member being pre-checked, whatever their own
/// class happens to be.
///
/// # What is redacted vs. preserved
/// Only `team_leader_pre_check.notes` is removed. `checked_by` and
/// `checked_at` remain — existence must stay visible per the rule
/// above. If `team_leader_pre_check` is absent (no pre-check recorded
/// yet) or `viewer` is `None` (caller could not determine the
/// requester, e.g. no authentication context), this function is a
/// no-op.
fn redact_team_leader_pre_check_for_viewer(value: &mut Value, viewer: Option<ObjectId>) {
    let Some(viewer) = viewer else {
        return;
    };
    let Some(object) = value.as_object_mut() else {
        return;
    };
    let Some(owner_id) = object.get("owner").and_then(object_id_array_to_object_id) else {
        return;
    };
    if owner_id != viewer {
        return;
    }
    if let Some(pre_check) = object
        .get_mut("team_leader_pre_check")
        .and_then(Value::as_object_mut)
    {
        pre_check.remove("notes");
    }
}

/// Same conversion as `object_id_array_to_uuid_string`, but returns the
/// raw `ObjectId` for equality comparison against `viewer` rather than
/// a display string — redaction needs to compare ids, not format them.
fn object_id_array_to_object_id(value: &Value) -> Option<ObjectId> {
    let array = value.as_array()?;
    if array.len() != 16 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, entry) in array.iter().enumerate() {
        bytes[i] = u8::try_from(entry.as_u64()?).ok()?;
    }
    Some(ObjectId(bytes))
}

/// Converts a JSON value shaped like `[u8; 16]` (16 numbers, each
/// 0-255) into the equivalent UUID string. Returns `None` for anything
/// else (already a string, wrong length, non-numeric, etc.) — this is
/// deliberately conservative: a value that doesn't match the expected
/// shape is left alone rather than guessed at.
fn object_id_array_to_uuid_string(value: &Value) -> Option<String> {
    let array = value.as_array()?;
    if array.len() != 16 {
        return None;
    }
    let mut bytes = [0u8; 16];
    for (i, entry) in array.iter().enumerate() {
        bytes[i] = u8::try_from(entry.as_u64()?).ok()?;
    }
    Some(uuid::Uuid::from_bytes(bytes).to_string())
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
