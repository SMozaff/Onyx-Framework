//! SQLite `Repository` implementation. Mirrors `PostgresRepository`.

use async_trait::async_trait;
use persistence_common::{text_to_value, timestamp_to_millis, value_to_text};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectId, ObjectVersion};
use query_application::{CommitResult, Loaded, Repository, RepositoryError, UnitOfWork};
use sqlx::SqlitePool;

pub struct SqliteRepository {
    pool: SqlitePool,
    aggregate_type: String,
}

impl SqliteRepository {
    pub fn new(pool: SqlitePool, aggregate_type: impl Into<String>) -> Self {
        Self {
            pool,
            aggregate_type: aggregate_type.into(),
        }
    }
}

#[async_trait]
impl Repository for SqliteRepository {
    async fn load(&self, id: &ObjectId) -> Result<Option<Loaded>, RepositoryError> {
        let id_bytes = id.0.to_vec();

        let row = sqlx::query_as::<_, (String, i64, i64, i64)>(
            r#"
            SELECT state, version, lifecycle_epoch, authority_epoch
            FROM aggregates
            WHERE id = ? AND aggregate_type = ?
            "#,
        )
        .bind(&id_bytes)
        .bind(&self.aggregate_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unknown(e.to_string()))?;

        let Some((state_text, version, lifecycle_epoch, authority_epoch)) = row else {
            return Ok(None);
        };

        let state = text_to_value(&state_text)
            .map_err(|e| RepositoryError::SerializationError(e.to_string()))?;

        Ok(Some(Loaded {
            aggregate: state,
            version: ObjectVersion(version as u64),
            lifecycle_epoch: LifecycleEpoch(lifecycle_epoch as u64),
            authority_epoch: AuthorityEpoch(authority_epoch as u64),
        }))
    }

    async fn commit(
        &self,
        aggregate_state: serde_json::Value,
        events: &[serde_json::Value],
        unit: &mut dyn UnitOfWork,
    ) -> Result<CommitResult, RepositoryError> {
        let conn_any = unit.connection().as_any();
        let sqlite_uow = conn_any
            .downcast_ref::<crate::unit_of_work::SqliteUnitOfWork>()
            .ok_or_else(|| {
                RepositoryError::Unknown(
                    "UnitOfWork passed to SqliteRepository::commit is not a SqliteUnitOfWork"
                        .to_string(),
                )
            })?;

        let id_bytes = extract_id_bytes(&aggregate_state, "id")
            .ok_or_else(|| RepositoryError::SerializationError("missing 'id' field".into()))?;
        let organization_id_bytes = unit.organization_id().0;
        let version = aggregate_state
            .get("version")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let lifecycle_epoch = aggregate_state
            .get("lifecycle_epoch")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let authority_epoch = aggregate_state
            .get("authority_epoch")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let now_millis = timestamp_to_millis(platform_kernel::Timestamp::now());

        sqlite_uow
            .stage_aggregate_upsert(crate::unit_of_work::StagedAggregateUpsert {
                id: id_bytes,
                aggregate_type: self.aggregate_type.clone(),
                organization_id: organization_id_bytes,
                version,
                lifecycle_epoch,
                authority_epoch,
                state: aggregate_state,
                updated_at_millis: now_millis,
            })
            .map_err(RepositoryError::Unknown)?;

        for event in events {
            unit.register_event(event.clone());
        }

        let _ = value_to_text; // referenced by sibling modules; kept for symmetry with Postgres adapter's import shape

        Ok(CommitResult {
            new_version: ObjectVersion(version as u64),
            new_lifecycle_epoch: Some(LifecycleEpoch(lifecycle_epoch as u64)),
            new_authority_epoch: Some(AuthorityEpoch(authority_epoch as u64)),
        })
    }
}

fn extract_id_bytes(value: &serde_json::Value, key: &str) -> Option<[u8; 16]> {
    let arr = value.get(key)?.as_array()?;
    if arr.len() != 16 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, b) in arr.iter().enumerate() {
        out[i] = b.as_u64()? as u8;
    }
    Some(out)
}
