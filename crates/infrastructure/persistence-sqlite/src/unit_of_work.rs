//! SQLite `UnitOfWork` implementation. Mirrors `persistence-postgres`'s
//! `PostgresUnitOfWork` design (buffer-then-flush-in-commit), adapted to
//! SQLite's column conventions per the "SQLite Schema Mapping" ruling
//! (DECISIONS.md S1): UUID -> BLOB, JSONB -> TEXT, TIMESTAMPTZ -> INTEGER
//! (Unix milliseconds, per ruling T1).

use async_trait::async_trait;
use persistence_common::{timestamp_to_millis, value_to_text};
use platform_kernel::{EventId, OperationId};
use query_application::{Connection, OutboxId, OutboxMessage, UnitOfWork, UnitOfWorkError};
use sqlx::{Sqlite, SqliteConnection, SqlitePool, Transaction};
use std::any::Any;
use std::sync::Mutex as StdMutex;
use tokio::sync::Mutex as TokioMutex;

struct StagedEvent {
    event_id: EventId,
    event_value: serde_json::Value,
}

struct StagedIdempotency {
    operation_id: OperationId,
    result_value: serde_json::Value,
}

pub struct StagedAggregateUpsert {
    pub id: [u8; 16],
    pub aggregate_type: String,
    pub organization_id: [u8; 16],
    pub version: i64,
    pub lifecycle_epoch: i64,
    pub authority_epoch: i64,
    pub state: serde_json::Value,
    pub updated_at_millis: i64,
}

struct Staged {
    events: Vec<StagedEvent>,
    outbox_messages: Vec<OutboxMessage>,
    idempotency: Vec<StagedIdempotency>,
    aggregate_upserts: Vec<StagedAggregateUpsert>,
}

pub struct SqliteUnitOfWork {
    txn: TokioMutex<Option<Transaction<'static, Sqlite>>>,
    staged: StdMutex<Staged>,
    organization_id: platform_kernel::OrganizationId,
}

impl SqliteUnitOfWork {
    pub async fn begin(
        pool: &SqlitePool,
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
impl UnitOfWork for SqliteUnitOfWork {
    fn organization_id(&self) -> platform_kernel::OrganizationId {
        self.organization_id
    }

    fn register_event(&mut self, event: serde_json::Value) -> EventId {
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

impl Connection for SqliteUnitOfWork {
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

async fn insert_domain_event(
    txn: &mut SqliteConnection,
    staged: &StagedEvent,
) -> Result<(), sqlx::Error> {
    let value = &staged.event_value;
    let event_id_bytes = staged.event_id.0;

    let aggregate_ref = value.get("aggregate_ref").cloned().unwrap_or_default();
    let aggregate_id_bytes = extract_id_bytes(&aggregate_ref, "id").unwrap_or([0u8; 16]);
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
    let vector_clock_text = value_to_text(&value.get("vector_clock").cloned().unwrap_or_default())
        .unwrap_or_else(|_| "{}".to_string());
    let operation_id_bytes = extract_id_bytes(value, "operation_id").unwrap_or([0u8; 16]);
    let correlation_id_bytes = extract_id_bytes(value, "correlation_id").unwrap_or([0u8; 16]);
    let causation_id_bytes = extract_id_bytes(value, "causation_id");
    let actor_text = value_to_text(&value.get("actor").cloned().unwrap_or_default())
        .unwrap_or_else(|_| "{}".to_string());
    let organization_id_bytes =
        extract_id_bytes(&aggregate_ref, "organization_id").unwrap_or([0u8; 16]);
    let payload_text = value_to_text(value).unwrap_or_else(|_| "{}".to_string());

    sqlx::query(
        r#"
        INSERT INTO domain_events (
            event_id, aggregate_id, aggregate_version, event_type, payload,
            occurred_at, vector_clock, operation_id, correlation_id,
            causation_id, actor, organization_id
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(event_id_bytes.to_vec())
    .bind(aggregate_id_bytes.to_vec())
    .bind(aggregate_version)
    .bind(event_type)
    .bind(payload_text)
    .bind(occurred_at_millis)
    .bind(vector_clock_text)
    .bind(operation_id_bytes.to_vec())
    .bind(correlation_id_bytes.to_vec())
    .bind(causation_id_bytes.map(|b| b.to_vec()))
    .bind(actor_text)
    .bind(organization_id_bytes.to_vec())
    .execute(&mut *txn)
    .await?;

    Ok(())
}

async fn insert_outbox_message(
    txn: &mut SqliteConnection,
    message: &OutboxMessage,
) -> Result<(), sqlx::Error> {
    let occurred_at_millis = timestamp_to_millis(message.occurred_at);
    // Ruling V1 (DECISIONS.md): previously
    // `serde_json::to_value(...).unwrap_or_default()` (silently degrading
    // to `Value::Null` on failure) piped into
    // `value_to_text(...).unwrap_or_else(|_| "{}".to_string())` (a SECOND
    // silent fallback) — a non-empty `VectorClock` would have been
    // written as the literal string `"{}"`, not even `null`, and no error
    // would ever surface. This masked a real, now-fixed defect in
    // `platform_kernel::VectorClock`'s JSON serialization (see
    // `causality.rs`). Both fallbacks are removed; a genuine failure now
    // propagates as `sqlx::Error::Protocol`, matching this crate's
    // existing convention for wrapping infrastructure failures at this
    // return type (translated to `UnitOfWorkError::CommitFailed` at the
    // call site, same as the Postgres adapter's identical fix).
    let vector_clock_value = serde_json::to_value(&message.vector_clock).map_err(|e| {
        sqlx::Error::Protocol(format!(
            "failed to serialize outbox message vector_clock: {e}"
        ))
    })?;
    let vector_clock_text = value_to_text(&vector_clock_value).map_err(|e| {
        sqlx::Error::Protocol(format!(
            "failed to encode outbox message vector_clock as text: {e}"
        ))
    })?;
    let payload_value: serde_json::Value = serde_json::from_slice(&message.payload)
        .unwrap_or_else(|_| serde_json::json!({ "raw": "unparseable" }));
    let payload_text = value_to_text(&payload_value).unwrap_or_else(|_| "{}".to_string());

    sqlx::query(
        r#"
        INSERT INTO outbox (
            event_id, event_type, aggregate_id, organization_id, payload,
            vector_clock, occurred_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(message.event_id.0.to_vec())
    .bind(&message.event_type)
    .bind(&message.aggregate_id)
    .bind(message.organization_id.0.to_vec())
    .bind(payload_text)
    .bind(vector_clock_text)
    .bind(occurred_at_millis)
    .execute(&mut *txn)
    .await?;

    Ok(())
}

async fn insert_idempotency(
    txn: &mut SqliteConnection,
    staged: &StagedIdempotency,
) -> Result<(), sqlx::Error> {
    let result_text = value_to_text(&staged.result_value).unwrap_or_else(|_| "{}".to_string());
    sqlx::query(
        r#"
        INSERT INTO idempotency (operation_id, result, created_at)
        VALUES (?, ?, ?)
        ON CONFLICT (operation_id) DO NOTHING
        "#,
    )
    .bind(staged.operation_id.0.to_vec())
    .bind(result_text)
    .bind(timestamp_to_millis(platform_kernel::Timestamp::now()))
    .execute(&mut *txn)
    .await?;
    Ok(())
}

async fn upsert_aggregate(
    txn: &mut SqliteConnection,
    upsert: &StagedAggregateUpsert,
) -> Result<(), sqlx::Error> {
    let state_text = value_to_text(&upsert.state).unwrap_or_else(|_| "{}".to_string());

    let result = sqlx::query(
        r#"
        INSERT INTO aggregates (id, aggregate_type, version, lifecycle_epoch, authority_epoch, state, updated_at, organization_id)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT (id) DO UPDATE SET
            version = excluded.version,
            lifecycle_epoch = excluded.lifecycle_epoch,
            authority_epoch = excluded.authority_epoch,
            state = excluded.state,
            updated_at = excluded.updated_at
        WHERE aggregates.version < excluded.version
        "#,
    )
    .bind(upsert.id.to_vec())
    .bind(&upsert.aggregate_type)
    .bind(upsert.version)
    .bind(upsert.lifecycle_epoch)
    .bind(upsert.authority_epoch)
    .bind(state_text)
    .bind(upsert.updated_at_millis)
    .bind(upsert.organization_id.to_vec())
    .execute(&mut *txn)
    .await?;

    if result.rows_affected() == 0 {
        return Err(sqlx::Error::RowNotFound);
    }

    Ok(())
}

/// Ruling F1 (`DECISIONS.md`): production `UnitOfWorkFactory` for SQLite.
/// See `PostgresUnitOfWorkFactory`'s doc comment (identical rationale) —
/// `SqliteUnitOfWork::begin(pool, organization_id)` already existed and
/// is already exercised by this crate's own integration tests; this is a
/// thin, logic-free wrapper making it reachable via the port trait.
pub struct SqliteUnitOfWorkFactory {
    pool: SqlitePool,
}

impl SqliteUnitOfWorkFactory {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl query_application::UnitOfWorkFactory for SqliteUnitOfWorkFactory {
    async fn create(
        &self,
        organization_id: platform_kernel::OrganizationId,
    ) -> Result<Box<dyn UnitOfWork>, UnitOfWorkError> {
        let unit = SqliteUnitOfWork::begin(&self.pool, organization_id).await?;
        Ok(Box::new(unit))
    }
}
