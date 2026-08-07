//! `AppState` — the composition root each client (desktop-shell,
//! mobile-core) constructs once at startup and holds for its process
//! lifetime, wiring [`CommandRegistry`], [`QueryRegistry`], [`EventBus`],
//! and [`SyncAgent`] together against a single SQLite database.
//!
//! # Provenance
//! Team Prompt 5 §4.1 step 5 ("Build the `CommandRegistry` and
//! `QueryRegistry` using the same application crates") and §4.2
//! ("mobile-core... constructs the same registries and sync agent as the
//! desktop shell"). Neither client builds its own copy of this wiring —
//! per ruling C1 (`DECISIONS.md`), it lives here once, in the shared
//! `client-composition` crate, and each client is a thin bridge to its
//! own UI transport (Tauri events vs. FFI callbacks).
//!
//! # Why SQLite specifically
//! Team Prompt 5 §1 states both clients use "a local SQLite store" —
//! desktop and mobile are each their own replica, syncing with each
//! other (and any cloud presence) via Increment 3/4's synchronization
//! engine, not talking to a shared Postgres backend directly. `AppState`
//! is therefore SQLite-specific by construction, unlike
//! `client-composition`'s other modules (`command_registry.rs` etc.),
//! which are storage-agnostic (`Arc<dyn Repository>`).

use std::sync::Arc;

use persistence_sqlite::{SqliteRepository, SqliteUnitOfWorkFactory};
use platform_kernel::{OrganizationId, ReplicaId};
use query_application::IdempotencyStore;
use sqlx::SqlitePool;
use sync_transport::placeholder_types::{CloudDiscovery, LocalDiscovery};
use sync_transport::{CompositeDiscovery, TransportSelector};

use crate::command_registry::CommandRegistry;
use crate::event_bus::EventBus;
use crate::handlers::{
    MissionCreationHandler, MissionDecisionHandler, TaskCreationHandler, TaskDecisionHandler,
};
use crate::query_registry::{LoadAggregateHandler, QueryRegistry};
use crate::sync_agent::{SyncAgent, SyncAgentConfig};

/// Every `MissionCommand` variant other than `CreateMission` (see
/// `mission_domain::command::MissionCommand`; 13 of its 14 total
/// variants). One `MissionDecisionHandler` instance correctly serves all
/// of them — it deserializes whichever variant the JSON payload actually
/// contains and dispatches through `Mission`'s own single `decide()`
/// match, so registering it repeatedly under every command_type string
/// is not duplicated logic, just duplicated routing keys.
const MISSION_DECISION_COMMANDS: &[&str] = &[
    "CreateBlueprintRevision",
    "SubmitBlueprint",
    "RequestApproval",
    "RejectApproval",
    "ActivateMission",
    "PauseMission",
    "OperationalHaltMission",
    "ResumeMission",
    "RestartMission",
    "TriggerReview",
    "CloseMission",
    "ArchiveMission",
    "CancelMission",
];

/// Every `TaskCommand` variant other than `CreateTask` (15 of 16 total).
/// See [`MISSION_DECISION_COMMANDS`]'s doc comment for why one handler
/// instance correctly serves all of them.
const TASK_DECISION_COMMANDS: &[&str] = &[
    "MarkReady",
    "AssignOwner",
    "ChangePriority",
    "AddDependency",
    "StartTask",
    "PauseTask",
    "BlockTask",
    "ResumeTask",
    "UnblockTask",
    "SubmitCompletion",
    "RejectTask",
    "ApproveTask",
    "CloseTask",
    "ReopenTask",
    "CancelTask",
];

/// Configuration `AppState::new` needs beyond the database pool itself.
/// Not specified by any prior increment.
pub struct AppStateConfig {
    pub local_replica: ReplicaId,
    pub organization_id: OrganizationId,
    pub sync_agent_config: SyncAgentConfig,
    /// Event bus broadcast channel capacity — see `EventBus::new`'s doc
    /// comment for what this bounds.
    pub event_bus_capacity: usize,
    /// Cloud Relay's endpoint URL (e.g. `wss://relay.onyx.example/v1`).
    /// Required — `TransportSelector` always includes Cloud Relay as the
    /// mandatory final fallback (Team Prompt 4 §3.3), so this cannot be
    /// `None` the way the local-radio transports below can.
    pub cloud_relay_endpoint: String,
    /// Provides the bearer token `CloudRelayTransport` authenticates
    /// with. No production implementation exists anywhere in the
    /// delivered workspace yet — `sync_transport::placeholder_types::
    /// StaticAuthorityProvider` is explicitly documented as test-only
    /// ("a trivial provider for tests: always returns a fixed token").
    /// Real credential sourcing (from `SecureStorage`, Increment 7's
    /// auth work, etc.) is the composing client's responsibility to
    /// supply here — `AppState` does not fabricate one.
    pub cloud_relay_auth_provider: Arc<dyn sync_transport::placeholder_types::AuthorityProvider>,
    /// Provides the actual socket implementation `CloudRelayTransport`
    /// connects over (e.g. `tokio-tungstenite`-backed WebSockets — see
    /// Team 4's `DECISIONS.md` §10 "no direct tokio in production code"
    /// seam, which is exactly what `RelaySocketFactory` exists to
    /// abstract). No production implementation exists anywhere in the
    /// delivered workspace yet either; same caller-supplies-it rationale
    /// as `cloud_relay_auth_provider`.
    pub cloud_relay_socket_factory: Arc<dyn sync_transport::cloud_relay::RelaySocketFactory>,
}

/// The composition root. Held for the client process's lifetime,
/// typically behind an `Arc` so Tauri command handlers / FFI functions
/// can each clone a reference into their own async task.
pub struct AppState {
    pub command_registry: Arc<CommandRegistry>,
    pub query_registry: Arc<QueryRegistry>,
    pub event_bus: Arc<EventBus>,
    pub sync_agent: Arc<SyncAgent>,

    /// Kept for tests / diagnostics that need to bypass the registries
    /// and query the raw pool directly (e.g. row-count assertions). Not
    /// otherwise used by the composition logic itself, which always goes
    /// through `command_registry`/`query_registry`.
    pub pool: SqlitePool,
}

impl AppState {
    /// Builds the full composition root against `pool`: repositories for
    /// `Mission` and `Task` (each aggregate-type-scoped, per the
    /// repository-scoping note on [`CreationHandler`](crate::CreationHandler)'s
    /// doc comment), the shared `UnitOfWorkFactory`, an in-memory
    /// idempotency store (see the caveat below), the fully-registered
    /// command/query registries for both aggregate types, a fresh
    /// `EventBus`, and a `SyncAgent` wired to a real (if currently
    /// empty) `TransportSelector`/`CompositeDiscovery` pair.
    ///
    /// # `IdempotencyStore` caveat (flagged, not silently resolved)
    /// No `IdempotencyStore` implementation exists anywhere in the
    /// delivered workspace — not a production adapter for either
    /// database, and not even a test mock (see `DECISIONS.md` ruling
    /// F1's finding). `AppState::new` therefore uses the same
    /// process-lifetime, in-memory implementation the end-to-end tests
    /// use (not persisted across restarts). This means idempotent
    /// duplicate-command detection does not survive a client restart —
    /// acceptable for now (it degrades to "occasionally re-executes a
    /// duplicate command after a restart" rather than corrupting data,
    /// since the optimistic-concurrency version check still protects
    /// against double-application), but a real gap a future increment
    /// or amendment should close with a genuine SQLite-backed store.
    pub fn new(pool: SqlitePool, config: AppStateConfig) -> Self {
        let mission_repo: Arc<dyn query_application::Repository> =
            Arc::new(SqliteRepository::new(pool.clone(), "mission"));
        let task_repo: Arc<dyn query_application::Repository> =
            Arc::new(SqliteRepository::new(pool.clone(), "task"));
        let unit_factory: Arc<dyn query_application::UnitOfWorkFactory> =
            Arc::new(SqliteUnitOfWorkFactory::new(pool.clone()));
        let idempotency_store: Arc<dyn IdempotencyStore> =
            Arc::new(InMemoryIdempotencyStore::new());

        let mut command_registry = CommandRegistry::new();
        command_registry.register_creation(
            "CreateMission",
            MissionCreationHandler::new(Arc::clone(&mission_repo), Arc::clone(&unit_factory)),
        );
        for command_type in MISSION_DECISION_COMMANDS {
            command_registry.register_decision(
                *command_type,
                MissionDecisionHandler::new(
                    Arc::clone(&mission_repo),
                    Arc::clone(&unit_factory),
                    Arc::clone(&idempotency_store),
                ),
            );
        }
        command_registry.register_creation(
            "CreateTask",
            TaskCreationHandler::new(Arc::clone(&task_repo), Arc::clone(&unit_factory)),
        );
        for command_type in TASK_DECISION_COMMANDS {
            command_registry.register_decision(
                *command_type,
                TaskDecisionHandler::new(
                    Arc::clone(&task_repo),
                    Arc::clone(&unit_factory),
                    Arc::clone(&idempotency_store),
                ),
            );
        }

        let mut query_registry = QueryRegistry::new();
        query_registry.register("GetMission", LoadAggregateHandler::new(Arc::clone(&mission_repo)));
        query_registry.register("GetTask", LoadAggregateHandler::new(Arc::clone(&task_repo)));

        let event_bus = Arc::new(EventBus::new(config.event_bus_capacity));

        // Real Discovery/TransportSelector, currently with no local radio
        // adapters wired in (Wi-Fi Direct / BLE / QUIC are Increment 4
        // deliverables whose platform-specific instantiation is
        // desktop-shell's/mobile-core's own concern, per Team Prompt 5
        // §3.5/§4.4 — CompositeDiscovery/TransportSelector here fall back
        // to Cloud Relay only until a client wires in the local ones).
        let discovery = Arc::new(CompositeDiscovery::new(CloudDiscovery::new(), LocalDiscovery::new()));
        let transport_selector = Arc::new(TransportSelector::new(
            None,
            None,
            None,
            Box::new(sync_transport::cloud_relay::CloudRelayTransport::new(
                config.cloud_relay_endpoint,
                config.cloud_relay_auth_provider,
                config.cloud_relay_socket_factory,
            )),
        ));

        let sync_agent = Arc::new(SyncAgent::new(
            config.local_replica,
            config.organization_id,
            // The outbox pump needs a real OutboxStore; SQLite's is
            // real and already delivered (Increment 2).
            Arc::new(persistence_sqlite::SqliteOutboxStore::new(pool.clone())),
            discovery,
            transport_selector,
            Arc::clone(&event_bus),
            config.sync_agent_config,
        ));

        Self {
            command_registry: Arc::new(command_registry),
            query_registry: Arc::new(query_registry),
            event_bus,
            sync_agent,
            pool,
        }
    }
}

/// Same in-memory `IdempotencyStore` used by the end-to-end tests; see
/// `AppState::new`'s doc comment for the caveat this represents.
struct InMemoryIdempotencyStore {
    store: std::sync::Mutex<std::collections::HashMap<platform_kernel::OperationId, serde_json::Value>>,
}

impl InMemoryIdempotencyStore {
    fn new() -> Self {
        Self {
            store: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl IdempotencyStore for InMemoryIdempotencyStore {
    async fn get(
        &self,
        operation_id: &platform_kernel::OperationId,
    ) -> Result<Option<serde_json::Value>, query_application::IdempotencyError> {
        Ok(self.store.lock().unwrap().get(operation_id).cloned())
    }

    async fn put(
        &self,
        operation_id: platform_kernel::OperationId,
        result: serde_json::Value,
    ) -> Result<(), query_application::IdempotencyError> {
        self.store.lock().unwrap().insert(operation_id, result);
        Ok(())
    }
}
