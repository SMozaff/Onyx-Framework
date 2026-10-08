//! Test utilities: in-memory mocks for the ports `synchronization-domain`
//! depends on (`Repository`, `UnitOfWorkFactory`, `Connection`,
//! `ConflictRepository`). Built against the real, delivered
//! `query-application` ports (not invented ones), since `EscalationService`
//! now goes through the real transactional write path (Architectural
//! Ruling Q3).

use std::any::Any;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use platform_kernel::{AuthorityEpoch, LifecycleEpoch, ObjectId, ObjectVersion, OrganizationId};
use query_application::{
    CommitResult, Connection, Loaded, Repository, RepositoryError, UnitOfWork, UnitOfWorkError,
    UnitOfWorkFactory,
};
use synchronization_domain::{ConflictRecord, ConflictRepository, ConflictRepositoryError};

/// A type-erased no-op connection handle for the mock `UnitOfWork`. Real
/// adapters downcast `Connection` to their own transaction type; the mock
/// has no real transaction, so `as_any()` returns itself and nothing
/// downcasts to it (nothing in these tests needs to).
pub struct MockConnection;

impl Connection for MockConnection {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// In-memory `UnitOfWork`: records every registered event/outbox message/
/// idempotency result, and "commits" by moving them into the owning
/// `MockRepository`'s shared logs on `commit()`.
pub struct MockUnitOfWork {
    organization_id: OrganizationId,
    registered_events: Vec<serde_json::Value>,
    committed: Arc<Mutex<CommittedLog>>,
}

#[derive(Default)]
struct CommittedLog {
    events: Vec<serde_json::Value>,
}

#[async_trait]
impl UnitOfWork for MockUnitOfWork {
    fn organization_id(&self) -> OrganizationId {
        self.organization_id
    }

    fn register_event(&mut self, event: serde_json::Value) -> platform_kernel::EventId {
        self.registered_events.push(event);
        platform_kernel::EventId::new_random()
    }

    fn register_outbox(
        &mut self,
        _message: query_application::OutboxMessage,
    ) -> query_application::OutboxId {
        // Mirrors the real, delivered persistence adapters (see
        // escalation.rs module doc comment): register_outbox is on the
        // port trait but nothing in the delivered Repository::commit()
        // implementations calls it. The mock matches that reality rather
        // than silently being more complete than the real adapters.
        query_application::OutboxId(0)
    }

    fn register_idempotency_result(
        &mut self,
        _operation_id: platform_kernel::OperationId,
        _result: serde_json::Value,
    ) {
    }

    async fn commit(&mut self) -> Result<(), UnitOfWorkError> {
        let mut log = self.committed.lock().unwrap();
        log.events.append(&mut self.registered_events);
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), UnitOfWorkError> {
        self.registered_events.clear();
        Ok(())
    }

    fn connection(&self) -> &dyn Connection {
        &MockConnection
    }
}

/// In-memory `Repository` + `UnitOfWorkFactory`: `commit()` stages events
/// into `self.committed_events` the same way the real SQLite adapter does
/// (registering them on the `UnitOfWork`, which the caller then commits).
pub struct MockRepository {
    aggregates: Mutex<std::collections::HashMap<[u8; 16], serde_json::Value>>,
    committed: Arc<Mutex<CommittedLog>>,
}

impl MockRepository {
    /// Create an empty mock repository.
    pub fn new() -> Self {
        Self {
            aggregates: Mutex::new(std::collections::HashMap::new()),
            committed: Arc::new(Mutex::new(CommittedLog::default())),
        }
    }

    /// All events committed so far, across every `MockUnitOfWork` this
    /// repository's `create()` calls produced.
    pub fn committed_events(&self) -> Vec<serde_json::Value> {
        self.committed.lock().unwrap().events.clone()
    }
}

impl Default for MockRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Repository for MockRepository {
    async fn load(&self, id: &ObjectId) -> Result<Option<Loaded>, RepositoryError> {
        let aggregates = self.aggregates.lock().unwrap();
        Ok(aggregates.get(&id.0).map(|state| Loaded {
            aggregate: state.clone(),
            version: ObjectVersion(0),
            lifecycle_epoch: LifecycleEpoch(0),
            authority_epoch: AuthorityEpoch(0),
        }))
    }

    async fn commit(
        &self,
        aggregate_state: serde_json::Value,
        events: &[serde_json::Value],
        unit: &mut dyn UnitOfWork,
    ) -> Result<CommitResult, RepositoryError> {
        if let Some(id) = aggregate_state.get("id").and_then(|v| v.as_array()) {
            if id.len() == 16 {
                let mut bytes = [0u8; 16];
                for (i, b) in id.iter().enumerate() {
                    bytes[i] = b.as_u64().unwrap_or(0) as u8;
                }
                self.aggregates
                    .lock()
                    .unwrap()
                    .insert(bytes, aggregate_state);
            }
        }
        for event in events {
            unit.register_event(event.clone());
        }
        Ok(CommitResult {
            new_version: ObjectVersion(1),
            new_lifecycle_epoch: Some(LifecycleEpoch(0)),
            new_authority_epoch: Some(AuthorityEpoch(0)),
        })
    }
}

#[async_trait]
impl UnitOfWorkFactory for MockRepository {
    async fn create(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Box<dyn UnitOfWork>, UnitOfWorkError> {
        Ok(Box::new(MockUnitOfWork {
            organization_id,
            registered_events: Vec::new(),
            committed: Arc::clone(&self.committed),
        }))
    }
}

/// In-memory `ConflictRepository` mock, seeded with a fixed set of
/// `ConflictRecord`s for `find_open_older_than` to filter against.
pub struct MockConflictRepository {
    /// The full seeded set of conflicts this mock filters over.
    pub conflicts: Arc<Mutex<Vec<ConflictRecord>>>,
}

impl MockConflictRepository {
    /// Create an empty mock repository.
    pub fn new() -> Self {
        Self {
            conflicts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a mock repository pre-seeded with the given conflicts.
    pub fn with_conflicts(conflicts: Vec<ConflictRecord>) -> Self {
        Self {
            conflicts: Arc::new(Mutex::new(conflicts)),
        }
    }
}

impl Default for MockConflictRepository {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConflictRepository for MockConflictRepository {
    async fn find_open_older_than(
        &self,
        since: platform_kernel::Timestamp,
    ) -> Result<Vec<ConflictRecord>, ConflictRepositoryError> {
        use synchronization_domain::ConflictStatus;
        Ok(self
            .conflicts
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.status == ConflictStatus::Open && c.created_at < since)
            .cloned()
            .collect())
    }
}
