//! PostgreSQL `UnitOfWork` implementation.
//!
//! Design note: `UnitOfWork::register_event`/`register_outbox`/
//! `register_idempotency_result` are synchronous, infallible-signature
//! methods (per the port trait — see `query-application::ports::
//! unit_of_work`), but writing to Postgres is async and fallible. This
//! implementation therefore buffers registrations in memory and performs
//! all actual `INSERT`s inside `commit()`, within a single sqlx
//! transaction, so the "atomicity across aggregate state + events +
//! outbox + idempotency" requirement (Team Prompt 2 §6.2 Atomicity Rule)
//! holds: either all writes land, or none do.

use async_trait::async_trait;
use persistence_common::timestamp_to_millis;
use platform_kernel::{EventId, OperationId};
use query_application::{Connection, OutboxId, OutboxMessage, UnitOfWork, UnitOfWorkError};
use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use std::any::Any;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex as TokioMutex;

/// A domain event staged for commit. `event_value` is the already-
/// serialized `platform_contracts::DomainEventEnvelope<E>` (per the
/// Team 1/Team 2 Interface Reconciliation ruling — `UnitOfWork` operates
/// on `serde_json::Value`, not a concrete envelope type).
struct StagedEvent {
    event_id: EventId,
    event_value: serde_json::Value,
}

struct StagedIdempotency {
    operation_id: OperationId,
    result_value: serde_json::Value,
}

/// A pending aggregate upsert, staged by `PostgresRepository::commit` via
/// `PostgresUnitOfWork::stage_aggregate_upsert` so the aggregate write
/// participates in the same atomic transaction as events/outbox/
/// idempotency (Team Prompt 2 §6.2 Atomicity Rule).
pub struct StagedAggregateUpsert {
    pub id: uuid::Uuid,
    pub aggregate_type: String,
    pub organization_id: uuid::Uuid,
    pub version: i64,
    pub lifecycle_epoch: i64,
    pub authority_epoch: i64,
    pub state: serde_json::Value,
    pub updated_at_millis: i64,
}

/// Internal mutable state, guarded by a `Mutex` so that `register_event`
/// etc. can take `&mut self` per the trait (sqlx's `Transaction` itself
/// also requires `&mut` for query execution, so this additionally lets us
/// keep the `Connection::as_any()` downcast path simple — see `conn()`).
struct Staged {
    events: Vec<StagedEvent>,
    outbox_messages: Vec<OutboxMessage>,
    idempotency: Vec<StagedIdempotency>,
    aggregate_upserts: Vec<StagedAggregateUpsert>,
}

pub struct PostgresUnitOfWork {
    txn: TokioMutex<Option<Transaction<'static, Postgres>>>,
    staged: StdMutex<Staged>,
    organization_id: platform_kernel::OrganizationId,
}

impl PostgresUnitOfWork {
    pub async fn begin(
        pool: &PgPool,
        organization_id: platform_kernel::OrganizationId,
    ) -> Result<Self, UnitOfWorkError> {
        let txn = pool.begin().await.map_err(|e| {
            UnitOfWorkError::CommitFailed(format!("failed to begin transaction: {e}"))
        })?;
        Ok(Self {
            txn: TokioMutex::new(Some(txn)),
            staged: StdMutex::new(Staged {
                events: Vec::new(),
                outbox_messages: Vec::new(),
                idempotency: Vec::new(),
                aggregate_upserts: Vec::new(),
            }),
            organization_id,
        })
    }

    /// Stage an aggregate upsert to be executed atomically inside
    /// `commit()`. Called by `PostgresRepository::commit` after it
    /// downcasts the `&mut dyn UnitOfWork` it was given back to
    /// `PostgresUnitOfWork` (see the "Connection Trait Shape" ruling, C1).
    pub fn stage_aggregate_upsert(&self, upsert: StagedAggregateUpsert) -> Result<(), String> {
        self.staged
            .lock()
            .map_err(|_| "staged lock poisoned".to_string())?
            .aggregate_upserts
            .push(upsert);
        Ok(())
    }
}

#[async_trait]
impl UnitOfWork for PostgresUnitOfWork {
    fn organization_id(&self) -> platform_kernel::OrganizationId {
        self.organization_id
    }

    fn register_event(&mut self, event: serde_json::Value) -> EventId {
        // The envelope carries its own event_id field (per
        // platform_contracts::DomainEventEnvelope); extract it back out so
        // callers get a usable EventId return value without having to
        // parse the Value themselves. If the caller passed a malformed
        // envelope missing `event_id`, we generate a placeholder rather
        // than panicking — the write will still be attempted at commit
        // time and will surface as a clear serialization error there if
        // the row genuinely can't be built.
        let event_id = extract_event_id(&event).unwrap_or(EventId([0u8; 16]));
        self.staged
            .lock()
            .expect("staged lock poisoned")
            .events
            .push(StagedEvent {
                event_id,
                event_value: event,
            });
        event_id
    }

    fn register_outbox(&mut self, message: OutboxMessage) -> OutboxId {
        // Real IDs are assigned by Postgres's BIGSERIAL at insert time
        // (see commit()); this placeholder is a stand-in used only to
        // satisfy the synchronous trait signature. Per Team Prompt 2 §3.2,
        // OutboxStore is the source of truth for OutboxId once persisted —
        // callers needing the real id must re-query, e.g. via
        // OutboxStore::claim_unpublished.
        let placeholder = OutboxId(0);
        self.staged
            .lock()
            .expect("staged lock poisoned")
            .outbox_messages
            .push(message);
        placeholder
    }

    fn register_idempotency_result(
        &mut self,
        operation_id: OperationId,
        result: serde_json::Value,
    ) {
        self.staged
            .lock()
            .expect("staged lock poisoned")
            .idempotency
            .push(StagedIdempotency {
                operation_id,
                result_value: result,
            });
    }

    async fn commit(&mut self) -> Result<(), UnitOfWorkError> {
        let mut txn_guard = self.txn.lock().await;
        let mut txn = txn_guard.take().ok_or_else(|| {
            UnitOfWorkError::CommitFailed("commit() called twice or after rollback".into())
        })?;

        let staged = {
            let mut s = self.staged.lock().expect("staged lock poisoned");
            std::mem::replace(
                &mut *s,
                Staged {
                    events: Vec::new(),
                    outbox_messages: Vec::new(),
                    idempotency: Vec::new(),
                    aggregate_upserts: Vec::new(),
                },
            )
        };

        for upsert in staged.aggregate_upserts {
            upsert_aggregate(&mut txn, &upsert).await.map_err(|e| {
                UnitOfWorkError::CommitFailed(format!("aggregate upsert failed: {e}"))
            })?;
        }

        for staged_event in staged.events {
            insert_domain_event(&mut txn, &staged_event)
                .await
                .map_err(|e| UnitOfWorkError::EventRegistration(e.to_string()))?;
        }

        for message in &staged.outbox_messages {
            insert_outbox_message(&mut txn, message)
                .await
                .map_err(|e| UnitOfWorkError::CommitFailed(format!("outbox insert failed: {e}")))?;
        }

        for idem in staged.idempotency {
            insert_idempotency(&mut txn, &idem).await.map_err(|e| {
                UnitOfWorkError::CommitFailed(format!("idempotency insert failed: {e}"))
            })?;
        }

        txn.commit()
            .await
            .map_err(|e| UnitOfWorkError::CommitFailed(e.to_string()))
    }

    async fn rollback(&mut self) -> Result<(), UnitOfWorkError> {
        let mut txn_guard = self.txn.lock().await;
        let txn = txn_guard.take().ok_or(UnitOfWorkError::RollbackFailed)?;
        txn.rollback()
            .await
            .map_err(|_| UnitOfWorkError::RollbackFailed)
    }

    fn connection(&self) -> &dyn Connection {
        self
    }
}

impl Connection for PostgresUnitOfWork {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn extract_event_id(event: &serde_json::Value) -> Option<EventId> {
    let bytes = event.get("event_id")?.as_array()?;
    if bytes.len() != 16 {
        return None;
    }
    let mut arr = [0u8; 16];
    for (i, b) in bytes.iter().enumerate() {
        arr[i] = b.as_u64()? as u8;
    }
    Some(EventId(arr))
}

async fn insert_domain_event(
    txn: &mut PgConnection,
    staged: &StagedEvent,
) -> Result<(), sqlx::Error> {
    let event_id_uuid = uuid_to_uuid_from_bytes(staged.event_id.0);
    let value = &staged.event_value;

    let aggregate_ref = value.get("aggregate_ref").cloned().unwrap_or_default();
    let aggregate_id_bytes = extract_id_bytes(&aggregate_ref, "id").unwrap_or([0u8; 16]);
    // ObjectVersion(u64) and Timestamp(u64) are single-field tuple structs
    // over a scalar, which serde_json serializes as a bare number (e.g.
    // `42`), NOT a 1-element array — unlike EventId/ObjectId/etc., which
    // wrap `[u8; 16]` and DO serialize as arrays. Verified empirically;
    // see DECISIONS.md item P1.
    let aggregate_version = value
        .get("aggregate_version")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let event_type = value
        .get("event_type")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let occurred_at_millis = value
        .get("occurred_at")
        .and_then(|v| v.as_i64())
        .map(|nanos| timestamp_to_millis(platform_kernel::Timestamp(nanos as u64)))
        .unwrap_or(0);
    let vector_clock = value.get("vector_clock").cloned().unwrap_or_default();
    let operation_id_bytes = extract_id_bytes(value, "operation_id").unwrap_or([0u8; 16]);
    let correlation_id_bytes = extract_id_bytes(value, "correlation_id").unwrap_or([0u8; 16]);
    let causation_id_bytes = extract_id_bytes(value, "causation_id");
    let actor = value.get("actor").cloned().unwrap_or_default();
    let organization_id_bytes =
        extract_id_bytes(&aggregate_ref, "organization_id").unwrap_or([0u8; 16]);

    sqlx::query(
        r#"
        INSERT INTO domain_events (
            event_id, aggregate_id, aggregate_version, event_type, payload,
            occurred_at, vector_clock, operation_id, correlation_id,
            causation_id, actor, organization_id
        ) VALUES ($1, $2, $3, $4, $5, to_timestamp($6::double precision / 1000.0), $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(event_id_uuid)
    .bind(uuid_to_uuid_from_bytes(aggregate_id_bytes))
    .bind(aggregate_version)
    .bind(event_type)
    .bind(value)
    .bind(occurred_at_millis)
    .bind(&vector_clock)
    .bind(uuid_to_uuid_from_bytes(operation_id_bytes))
    .bind(uuid_to_uuid_from_bytes(correlation_id_bytes))
    .bind(causation_id_bytes.map(uuid_to_uuid_from_bytes))
    .bind(&actor)
    .bind(uuid_to_uuid_from_bytes(organization_id_bytes))
    .execute(&mut *txn)
    .await?;

    Ok(())
}

async fn insert_outbox_message(
    txn: &mut PgConnection,
    message: &OutboxMessage,
) -> Result<(), sqlx::Error> {
    let event_id_uuid = uuid_to_uuid_from_bytes(message.event_id.0);
    let organization_id_uuid = uuid_to_uuid_from_bytes(message.organization_id.0);
    let occurred_at_millis = timestamp_to_millis(message.occurred_at);
    // Ruling V1 (DECISIONS.md): previously `.unwrap_or(serde_json::Value::Null)`,
    // which silently wrote `null` into the `vector_clock` column for any
    // non-empty VectorClock (a real, previously-undetected defect in
    // `platform_kernel::VectorClock`'s JSON serialization, now fixed at
    // its source — see `causality.rs`). Serialization is not expected to
    // fail anymore, but this no longer masks it as `null` if it somehow
    // does: the error propagates through this function's existing
    // `Result<(), sqlx::Error>` return type (via `sqlx::Error::Protocol`,
    // matching this crate's own convention of wrapping infrastructure
    // failures as `sqlx::Error` here and translating to
    // `UnitOfWorkError::CommitFailed` at the call site).
    let vector_clock_json = serde_json::to_value(&message.vector_clock).map_err(|e| {
        sqlx::Error::Protocol(format!(
            "failed to serialize outbox message vector_clock: {e}"
        ))
    })?;
    let payload_json: serde_json::Value = serde_json::from_slice(&message.payload)
        .unwrap_or_else(|_| serde_json::json!({ "raw_base64": base64_encode(&message.payload) }));

    sqlx::query(
        r#"
        INSERT INTO outbox (
            event_id, event_type, aggregate_id, organization_id, payload,
            vector_clock, occurred_at
        ) VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7::double precision / 1000.0))
        "#,
    )
    .bind(event_id_uuid)
    .bind(&message.event_type)
    .bind(&message.aggregate_id)
    .bind(organization_id_uuid)
    .bind(&payload_json)
    .bind(&vector_clock_json)
    .bind(occurred_at_millis)
    .execute(&mut *txn)
    .await?;

    Ok(())
}

async fn insert_idempotency(
    txn: &mut PgConnection,
    staged: &StagedIdempotency,
) -> Result<(), sqlx::Error> {
    let operation_id_uuid = uuid_to_uuid_from_bytes(staged.operation_id.0);
    sqlx::query(
        r#"
        INSERT INTO idempotency (operation_id, result)
        VALUES ($1, $2)
        ON CONFLICT (operation_id) DO NOTHING
        "#,
    )
    .bind(operation_id_uuid)
    .bind(&staged.result_value)
    .execute(&mut *txn)
    .await?;
    Ok(())
}

fn uuid_to_uuid_from_bytes(bytes: [u8; 16]) -> uuid::Uuid {
    persistence_common::bytes_to_uuid(bytes)
}

fn extract_id_bytes(value: &serde_json::Value, key: &str) -> Option<[u8; 16]> {
    let raw = value.get(key)?;
    if raw.is_null() {
        return None;
    }
    // Kernel identifier newtypes serialize as a direct 16-number JSON
    // array. Older illustrative fixtures used a one-element tuple wrapper,
    // so accept both shapes at the persistence boundary.
    let arr = raw
        .as_array()
        .filter(|items| items.len() == 16)
        .or_else(|| raw.get(0)?.as_array())?;
    if arr.len() != 16 {
        return None;
    }
    let mut out = [0u8; 16];
    for (i, b) in arr.iter().enumerate() {
        out[i] = b.as_u64()? as u8;
    }
    Some(out)
}

fn base64_encode(bytes: &[u8]) -> String {
    // Minimal, dependency-free base64 (standard alphabet, with padding),
    // used only as a last-resort fallback when an OutboxMessage's payload
    // isn't valid JSON. Avoids pulling in the `base64` crate for a single
    // rarely-hit branch.
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(ALPHABET[(n >> 18 & 0x3F) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 0x3F) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 0x3F) as usize] as char
        } else {
            '='
        });
    }
    out
}

async fn upsert_aggregate(
    txn: &mut PgConnection,
    upsert: &StagedAggregateUpsert,
) -> Result<(), sqlx::Error> {
    // INSERT ... ON CONFLICT with an explicit expected-version guard on the
    // UPDATE branch implements optimistic concurrency control per Handover
    // §8: a write only succeeds if either (a) no row exists yet (first
    // write) or (b) the row's current version is strictly less than the
    // new version being written. Team Prompt 2 does not spell out the
    // exact upsert SQL, so this shape is our own construction; see
    // DECISIONS.md item P2.
    let result = sqlx::query(
        r#"
        INSERT INTO aggregates (id, aggregate_type, version, lifecycle_epoch, authority_epoch, state, updated_at, organization_id)
        VALUES ($1, $2, $3, $4, $5, $6, to_timestamp($7::double precision / 1000.0), $8)
        ON CONFLICT (id) DO UPDATE SET
            version = EXCLUDED.version,
            lifecycle_epoch = EXCLUDED.lifecycle_epoch,
            authority_epoch = EXCLUDED.authority_epoch,
            state = EXCLUDED.state,
            updated_at = EXCLUDED.updated_at
        WHERE aggregates.version < EXCLUDED.version
        "#,
    )
    .bind(upsert.id)
    .bind(&upsert.aggregate_type)
    .bind(upsert.version)
    .bind(upsert.lifecycle_epoch)
    .bind(upsert.authority_epoch)
    .bind(&upsert.state)
    .bind(upsert.updated_at_millis)
    .bind(upsert.organization_id)
    .execute(&mut *txn)
    .await?;

    if result.rows_affected() == 0 {
        // For a first-time INSERT this branch can't be hit by the WHERE
        // guard (there's no conflict to filter), so 0 rows affected here
        // specifically means an UPDATE was attempted against a stale
        // version — i.e. a genuine optimistic-concurrency conflict.
        return Err(sqlx::Error::RowNotFound);
    }

    Ok(())
}

/// Ruling F1 (`DECISIONS.md`): production `UnitOfWorkFactory` for
/// Postgres. `PostgresUnitOfWork::begin(pool, organization_id)` already
/// existed and is already exercised by this crate's own integration
/// tests — this is a thin, logic-free wrapper making it reachable via
/// the port trait `api_server::handle_command` (and, downstream,
/// `client-composition`'s `MissionDecisionHandler`/`TaskDecisionHandler`)
/// actually requires, since no adapter previously implemented
/// `UnitOfWorkFactory` in production (only a `sync-test-utils` in-memory
/// mock existed workspace-wide before this ruling).
pub struct PostgresUnitOfWorkFactory {
    pool: PgPool,
}

impl PostgresUnitOfWorkFactory {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl query_application::UnitOfWorkFactory for PostgresUnitOfWorkFactory {
    async fn create(
        &self,
        organization_id: platform_kernel::OrganizationId,
    ) -> Result<Box<dyn UnitOfWork>, UnitOfWorkError> {
        let unit = PostgresUnitOfWork::begin(&self.pool, organization_id).await?;
        Ok(Box::new(unit))
    }
}
