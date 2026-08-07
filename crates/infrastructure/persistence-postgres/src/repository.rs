//! PostgreSQL `Repository` implementation.
//!
//! Per the Team 1/Team 2 Interface Reconciliation ruling, this repository
//! is non-generic and works entirely in `serde_json::Value` for aggregate
//! state; it does not know about any concrete aggregate type (`Mission`,
//! `Task`, etc.) — that knowledge lives in the domain crates and the
//! command handler that calls this repository.

use async_trait::async_trait;
use persistence_common::{bytes_to_uuid, timestamp_to_millis};
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectId, ObjectVersion};
use query_application::{CommitResult, Loaded, Repository, RepositoryError, UnitOfWork};
use sqlx::PgPool;

pub struct PostgresRepository {
    pool: PgPool,
    /// The `aggregate_type` discriminant stored alongside every row (e.g.
    /// `"mission"`). Team Prompt 2 §4.1's `aggregates` table has a single
    /// `aggregate_type TEXT` column shared across all aggregate kinds, so
    /// one physical table serves every aggregate type; this repository
    /// instance is scoped to one type at construction time.
    aggregate_type: String,
}

impl PostgresRepository {
    pub fn new(pool: PgPool, aggregate_type: impl Into<String>) -> Self {
        Self {
            pool,
            aggregate_type: aggregate_type.into(),
        }
    }
}

#[async_trait]
impl Repository for PostgresRepository {
    async fn load(&self, id: &ObjectId) -> Result<Option<Loaded>, RepositoryError> {
        let id_uuid = bytes_to_uuid(id.0);

        let row = sqlx::query!(
            r#"
            SELECT state, version, lifecycle_epoch, authority_epoch
            FROM aggregates
            WHERE id = $1 AND aggregate_type = $2
            "#,
            id_uuid,
            self.aggregate_type,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RepositoryError::Unknown(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        Ok(Some(Loaded {
            aggregate: row.state,
            version: ObjectVersion(row.version as u64),
            lifecycle_epoch: LifecycleEpoch(row.lifecycle_epoch as u64),
            authority_epoch: AuthorityEpoch(row.authority_epoch as u64),
        }))
    }

    async fn commit(
        &self,
        aggregate_state: serde_json::Value,
        events: &[serde_json::Value],
        unit: &mut dyn UnitOfWork,
    ) -> Result<CommitResult, RepositoryError> {
        // Downcast to the concrete PostgresUnitOfWork so we can share its
        // transaction, per the "Connection Trait Shape" ruling (C1).
        let conn_any = unit.connection().as_any();
        let pg_uow = conn_any
            .downcast_ref::<crate::unit_of_work::PostgresUnitOfWork>()
            .ok_or_else(|| {
                RepositoryError::Unknown(
                    "UnitOfWork passed to PostgresRepository::commit is not a PostgresUnitOfWork"
                        .to_string(),
                )
            })?;

        // Extract identity/versioning fields from the serialized aggregate
        // state. Per Team Prompt 2 §1 Mission, the concrete aggregate shape
        // is a domain-crate concern; the repository only needs a few
        // well-known fields to route the upsert, which it reads out of the
        // JSON envelope the domain layer is expected to produce (id,
        // version, lifecycle_epoch, authority_epoch — matching the
        // aggregates table's own columns).
        //
        // organization_id is NOT read from aggregate_state: per
        // Architectural Ruling "Organization ID in Persistence"
        // (DECISIONS.md O1), Mission/Task aggregates carry no
        // organization_id field of their own — it is a tenant-boundary
        // concept living at the command-envelope/UnitOfWork level, sourced
        // here via `unit.organization_id()`.
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

        let id_uuid = bytes_to_uuid(id_bytes);
        let organization_id_uuid = bytes_to_uuid(organization_id_bytes);
        let now_millis = timestamp_to_millis(platform_kernel::Timestamp::now());

        // We can't easily get a &mut on pg_uow's private transaction field
        // from here (it's behind a Mutex<Option<Transaction>> owned by
        // PostgresUnitOfWork), so instead the upsert is executed against
        // the same pool but is registered as a *staged event/side-effect*
        // is not appropriate here — the aggregate write must be part of
        // the same atomic commit as events/outbox/idempotency. We expose a
        // dedicated method on PostgresUnitOfWork for this.
        pg_uow
            .stage_aggregate_upsert(crate::unit_of_work::StagedAggregateUpsert {
                id: id_uuid,
                aggregate_type: self.aggregate_type.clone(),
                organization_id: organization_id_uuid,
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
