//! Native Slint composition root.
//!
//! Keeps authentication and relay authority in Rust. The UI receives only
//! non-secret identity fields. Session authority is the shared
//! `desktop-session` crate: the refresh token lives in the OS keyring, and the
//! access token stays in memory. Neither is exposed as a Slint property.
//!
//! Offline outbox: the local SQLite store opens at startup. After a login the
//! app state for that organization is built and its sync agent starts, so
//! outbox rows upload to the Worker. Logout stops the sync agent.

use std::path::PathBuf;
use std::sync::Arc;

use client_composition::app_state::{AppState, AppStateConfig};
use platform_kernel::{OrganizationId, ReplicaId};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use sync_transport::{cloud_relay::CloudRelayTransport, placeholder_types::AuthorityProvider};
use tokio::sync::Mutex;

use desktop_session::{
    secure_storage::{keyring_adapter::KeyringSecureStorage, SecureStorage},
    session::{self, HttpSessionApi, SessionError},
    session_authority::{SessionAuthority, SessionBearerProvider, SessionSummary},
};

use crate::device;
use relay_websocket_adapter::TungsteniteRelaySocketFactory;

/// The same canonical API origin used by the current desktop-shell login flow.
pub const ONYX_API_ORIGIN: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

pub struct SlintComposition {
    pub session: Arc<SessionAuthority>,
    pub relay: Arc<CloudRelayTransport>,
    replica: ReplicaId,
    data_dir: PathBuf,
    pool: SqlitePool,
    outbox: Mutex<Option<OutboxRuntime>>,
}

/// The running app state for the signed-in organization. Dropping it does
/// not stop the sync task, so `stop_outbox` aborts the task explicitly.
struct OutboxRuntime {
    _state: Arc<AppState>,
    task: tokio::task::JoinHandle<()>,
}

impl SlintComposition {
    /// Opens the device database, applies migrations and builds the session
    /// service and relay. Must be awaited inside the Tokio runtime.
    pub async fn open() -> Result<Arc<Self>, String> {
        let data_dir = device::data_dir()?;
        std::fs::create_dir_all(&data_dir).map_err(|e| format!("create data folder: {e}"))?;
        let replica = device::load_or_create_replica_id(&data_dir)?;

        let options = SqliteConnectOptions::new()
            .filename(data_dir.join("onyx.sqlite"))
            .create_if_missing(true)
            // Foreign keys are off by default in SQLite; the schema depends on them.
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .map_err(|e| format!("open database: {e}"))?;
        sqlx::migrate!("../../../migrations/sqlite")
            .run(&pool)
            .await
            .map_err(|e| format!("apply migrations: {e}"))?;

        let api = Arc::new(
            HttpSessionApi::new(ONYX_API_ORIGIN).map_err(|e| format!("session api: {e}"))?,
        );
        // Separate keyring service from the Tauri shell so both can be installed on
        // one device without overwriting each other's session (decision-log).
        let storage: Arc<dyn SecureStorage> =
            Arc::new(KeyringSecureStorage::for_service("com.onyx.platform.slint"));
        let session = SessionAuthority::new(ONYX_API_ORIGIN, api, Arc::clone(&storage));
        let authority: Arc<dyn AuthorityProvider> =
            Arc::new(SessionBearerProvider::new(Arc::clone(&session)));
        let socket_factory = Arc::new(TungsteniteRelaySocketFactory::with_cancellation(
            replica,
            session.connection_cancellation_source(),
        ));
        let relay = Arc::new(CloudRelayTransport::new(
            ONYX_API_ORIGIN,
            authority,
            socket_factory,
        ));

        let composition = Arc::new(Self {
            session,
            relay,
            replica,
            data_dir,
            pool,
            outbox: Mutex::new(None),
        });
        composition.restore_session(&storage).await;
        Ok(composition)
    }

    /// Restores a stored refresh token from the keyring at startup. On success
    /// the sync agent starts for that organization, as after a fresh sign-in.
    async fn restore_session(&self, storage: &Arc<dyn SecureStorage>) {
        match session::load(storage).await {
            Ok(Some(stored)) => {
                if let Some(summary) = self.session.restore(stored).await {
                    self.start_outbox(&summary.organization_id.to_string()).await;
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %e, "stored session could not be read"),
        }
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SessionSummary, SessionError> {
        let outcome = self.session.sign_in(username, password).await?;
        if !outcome.persisted {
            tracing::warn!("session is active in memory only; keyring save failed");
        }
        let summary = outcome.summary;
        self.start_outbox(&summary.organization_id.to_string()).await;
        Ok(summary)
    }

    pub async fn logout(&self) {
        self.stop_outbox().await;
        self.session.sign_out().await;
    }

    /// Builds the app state for `organization_id` and starts its sync agent.
    /// A failure is logged; login still succeeds and rows stay in the local
    /// outbox until a later attempt.
    async fn start_outbox(&self, organization_id: &str) {
        self.stop_outbox().await;
        let org = match OrganizationId::from_uuid_str(organization_id) {
            Ok(org) => org,
            Err(e) => {
                tracing::warn!(error = %e, "outbox not started: invalid organization id");
                return;
            }
        };

        let config = AppStateConfig {
            local_replica: self.replica,
            organization_id: org,
            sync_agent_config: client_composition::sync_agent::SyncAgentConfig::default(),
            event_bus_capacity: 1024,
            cloud_relay_endpoint: ONYX_API_ORIGIN.replacen("https://", "wss://", 1),
            local_discovery: None,
            cloud_relay_auth_provider: Arc::new(SessionBearerProvider::new(Arc::clone(&self.session))),
            cloud_relay_socket_factory: Arc::new(TungsteniteRelaySocketFactory::with_cancellation(
                self.replica,
                self.session.connection_cancellation_source(),
            )),
            blob_store_root: self.data_dir.join("blobs"),
            owner_authority: None,
            cloud_outbox_endpoint: Some(format!("{ONYX_API_ORIGIN}/api/sync/outbox")),
        };

        match AppState::new(self.pool.clone(), config).await {
            Ok(state) => {
                let state = Arc::new(state);
                let agent = Arc::clone(&state.sync_agent);
                let task = tokio::spawn(async move { agent.run().await });
                *self.outbox.lock().await = Some(OutboxRuntime {
                    _state: state,
                    task,
                });
            }
            Err(e) => tracing::warn!(error = %e, "outbox not started"),
        }
    }

    async fn stop_outbox(&self) {
        if let Some(runtime) = self.outbox.lock().await.take() {
            runtime.task.abort();
        }
    }
}
