//! `QueryRegistry` — maps a `QueryEnvelope.query_type` string to the
//! handler logic for that query.
//!
//! # Provenance
//! Team Prompt 5 §3.2 (illustrative — R1/R2). The only real query
//! primitive Increment 1–4 delivered is `api_server::load_aggregate(id,
//! repo)` — "load from the repository and return", per that file's own
//! doc comment; no query-side projections, filters, or list queries exist
//! yet. This registry is therefore deliberately thin: it supports
//! single-aggregate-by-id lookups now, with room to register richer query
//! handlers as later increments (or a future amendment) add real
//! projection infrastructure.

use std::collections::HashMap;
use std::sync::Arc;

use platform_kernel::ObjectId;
use query_application::{Loaded, Repository, RepositoryError};
use serde::{Deserialize, Serialize};

/// A JSON-serializable envelope for a query request. Not defined by any
/// prior increment (Team Prompt 5 §3.2 references `QueryEnvelope` by name
/// only); shaped to carry what `load_aggregate` needs plus a
/// `query_type` string for registry dispatch, mirroring
/// `CommandEnvelope`'s `command_type` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEnvelope {
    pub query_type: String,
    pub target_id: ObjectId,
}

/// The result of a successful query. Wraps `Loaded` (the only real
/// query-side shape Increment 2 defined) as JSON so the Tauri/FFI boundary
/// has a single response type regardless of query kind.
pub type QueryResponse = serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum QueryDispatchError {
    #[error("no handler registered for query type {0:?}")]
    UnknownQueryType(String),
    #[error("repository error: {0}")]
    Repository(#[from] RepositoryError),
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// A handler for one query type. Registered once per aggregate type (each
/// bound to that type's own `Repository` instance — recall
/// `SqliteRepository::new(pool, aggregate_type)` is scoped per type).
#[async_trait::async_trait]
pub trait QueryHandler: Send + Sync {
    async fn handle_query(&self, target_id: ObjectId) -> Result<Option<Loaded>, RepositoryError>;
}

/// The default handler for a plain "load aggregate by id" query: wraps
/// `api_server::load_aggregate` unmodified.
pub struct LoadAggregateHandler {
    repo: Arc<dyn Repository>,
}

impl LoadAggregateHandler {
    pub fn new(repo: Arc<dyn Repository>) -> Self {
        Self { repo }
    }
}

#[async_trait::async_trait]
impl QueryHandler for LoadAggregateHandler {
    async fn handle_query(&self, target_id: ObjectId) -> Result<Option<Loaded>, RepositoryError> {
        api_server::load_aggregate(&target_id, Arc::clone(&self.repo)).await
    }
}

pub struct QueryRegistry {
    handlers: HashMap<String, Box<dyn QueryHandler>>,
}

impl QueryRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(
        &mut self,
        query_type: impl Into<String>,
        handler: impl QueryHandler + 'static,
    ) -> &mut Self {
        self.handlers.insert(query_type.into(), Box::new(handler));
        self
    }

    pub async fn dispatch(&self, query: QueryEnvelope) -> Result<QueryResponse, QueryDispatchError> {
        let handler = self
            .handlers
            .get(&query.query_type)
            .ok_or_else(|| QueryDispatchError::UnknownQueryType(query.query_type.clone()))?;

        let loaded = handler.handle_query(query.target_id).await?;
        Ok(serde_json::to_value(loaded.map(LoadedJson::from))?)
    }
}

impl Default for QueryRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON-serializable mirror of `Loaded` (which itself does not derive
/// `Serialize`/`Deserialize` in `query-application` — it's an internal
/// port type). Constructed at the registry's JSON boundary only.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoadedJson {
    aggregate: serde_json::Value,
    version: platform_kernel::ObjectVersion,
    lifecycle_epoch: platform_kernel::LifecycleEpoch,
    authority_epoch: platform_kernel::AuthorityEpoch,
}

impl From<Loaded> for LoadedJson {
    fn from(l: Loaded) -> Self {
        Self {
            aggregate: l.aggregate,
            version: l.version,
            lifecycle_epoch: l.lifecycle_epoch,
            authority_epoch: l.authority_epoch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use query_application::UnitOfWork;
    use std::collections::HashMap as StdHashMap;
    use std::sync::Mutex;

    /// A minimal in-memory `Repository` for testing `QueryRegistry`
    /// without a real database. `commit` is unimplemented (panics) since
    /// no test here needs it — only `load`, which
    /// `LoadAggregateHandler`/`api_server::load_aggregate` calls.
    struct InMemoryRepository {
        store: Mutex<StdHashMap<ObjectId, Loaded>>,
    }

    impl InMemoryRepository {
        fn new() -> Self {
            Self {
                store: Mutex::new(StdHashMap::new()),
            }
        }

        fn seed(&self, id: ObjectId, loaded: Loaded) {
            self.store.lock().unwrap().insert(id, loaded);
        }
    }

    #[async_trait::async_trait]
    impl Repository for InMemoryRepository {
        async fn load(&self, id: &ObjectId) -> Result<Option<Loaded>, RepositoryError> {
            Ok(self.store.lock().unwrap().get(id).cloned())
        }

        async fn commit(
            &self,
            _aggregate_state: serde_json::Value,
            _events: &[serde_json::Value],
            _unit: &mut dyn UnitOfWork,
        ) -> Result<query_application::CommitResult, RepositoryError> {
            unimplemented!("not exercised by these tests")
        }
    }

    fn sample_loaded() -> Loaded {
        Loaded {
            aggregate: serde_json::json!({"name": "Test Mission"}),
            version: platform_kernel::ObjectVersion(3),
            lifecycle_epoch: platform_kernel::LifecycleEpoch(0),
            authority_epoch: platform_kernel::AuthorityEpoch(0),
        }
    }

    #[tokio::test]
    async fn dispatch_returns_loaded_aggregate_as_json() {
        let repo = Arc::new(InMemoryRepository::new());
        let id = ObjectId::new_random();
        repo.seed(id, sample_loaded());

        let mut registry = QueryRegistry::new();
        registry.register("GetMission", LoadAggregateHandler::new(repo));

        let response = registry
            .dispatch(QueryEnvelope {
                query_type: "GetMission".to_string(),
                target_id: id,
            })
            .await
            .unwrap();

        assert_eq!(response["aggregate"]["name"], serde_json::json!("Test Mission"));
        assert_eq!(response["version"], serde_json::json!(3));
    }

    #[tokio::test]
    async fn dispatch_returns_null_for_missing_aggregate() {
        let repo = Arc::new(InMemoryRepository::new());
        let mut registry = QueryRegistry::new();
        registry.register("GetMission", LoadAggregateHandler::new(repo));

        let response = registry
            .dispatch(QueryEnvelope {
                query_type: "GetMission".to_string(),
                target_id: ObjectId::new_random(),
            })
            .await
            .unwrap();

        assert_eq!(response, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn dispatch_rejects_unknown_query_type() {
        let registry = QueryRegistry::new();
        let err = registry
            .dispatch(QueryEnvelope {
                query_type: "NoSuchQuery".to_string(),
                target_id: ObjectId::new_random(),
            })
            .await
            .unwrap_err();

        match err {
            QueryDispatchError::UnknownQueryType(t) => assert_eq!(t, "NoSuchQuery"),
            other => panic!("expected UnknownQueryType, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn two_aggregate_types_can_be_registered_with_independent_repositories() {
        let mission_repo = Arc::new(InMemoryRepository::new());
        let task_repo = Arc::new(InMemoryRepository::new());
        let mission_id = ObjectId::new_random();
        let task_id = ObjectId::new_random();
        mission_repo.seed(mission_id, sample_loaded());
        task_repo.seed(task_id, sample_loaded());

        let mut registry = QueryRegistry::new();
        registry
            .register("GetMission", LoadAggregateHandler::new(mission_repo))
            .register("GetTask", LoadAggregateHandler::new(task_repo));

        let mission_response = registry
            .dispatch(QueryEnvelope {
                query_type: "GetMission".to_string(),
                target_id: mission_id,
            })
            .await
            .unwrap();
        let task_response = registry
            .dispatch(QueryEnvelope {
                query_type: "GetTask".to_string(),
                target_id: task_id,
            })
            .await
            .unwrap();

        assert_ne!(mission_response, serde_json::Value::Null);
        assert_ne!(task_response, serde_json::Value::Null);

        // Cross-check: a mission id looked up under GetTask (wrong repo)
        // correctly finds nothing, proving the two registrations are
        // actually backed by independent repositories, not a shared one.
        let cross_lookup = registry
            .dispatch(QueryEnvelope {
                query_type: "GetTask".to_string(),
                target_id: mission_id,
            })
            .await
            .unwrap();
        assert_eq!(cross_lookup, serde_json::Value::Null);
    }
}
