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
    session::{self, SessionError},
    session_authority::{
        http_api_factory, SessionAuthority, SessionBearerProvider, SessionSummary,
    },
};
use serde_json::Value;

use crate::device;
use relay_websocket_adapter::TungsteniteRelaySocketFactory;

/// The same canonical API origin used by the current desktop-shell login flow.
/// Build-time override via `ONYX_API_ORIGIN`. Unset builds use the production origin.
pub const ONYX_API_ORIGIN: &str = match option_env!("ONYX_API_ORIGIN") {
    Some(origin) => origin,
    None => "https://onyx-framework.soheil-mozaffari.workers.dev",
};

/// Stable login failures for the UI. The Worker remains the authority; these
/// are only the host's display categories, never a substitute for server policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginFailure {
    OrganizationNotFound,
    InvalidOrganizationServer,
    InvalidCredentials,
    Network,
    Server,
}

/// Stable password-change failures for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasswordFailure {
    CurrentPasswordIncorrect,
    WeakPassword,
    PasswordUnchanged,
    SessionExpired,
    Network,
    Server,
}

/// Maps an HTTP(S) origin to the relay endpoint used by the local outbox.
pub fn relay_ws_endpoint(server: &str) -> String {
    if let Some(rest) = server.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = server.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        server.to_string()
    }
}

fn password_failure_code(code: &str) -> PasswordFailure {
    match code {
        "CURRENT_PASSWORD_INCORRECT" => PasswordFailure::CurrentPasswordIncorrect,
        "WEAK_PASSWORD" | "INVALID_PASSWORD_CHANGE" => PasswordFailure::WeakPassword,
        "PASSWORD_UNCHANGED" => PasswordFailure::PasswordUnchanged,
        "PASSWORD_CHANGE_REQUIRED" => PasswordFailure::SessionExpired,
        _ => PasswordFailure::Server,
    }
}

/// Reads the organization registry response. A null/empty `server_url` means
/// the cloud server itself; only http(s) addresses are accepted.
fn server_from_lookup(body: &str, cloud_base: &str) -> Result<String, LoginFailure> {
    let value: Value = serde_json::from_str(body).map_err(|_| LoginFailure::Server)?;
    let Some(url) = value.get("server_url").and_then(Value::as_str) else {
        return Ok(cloud_base.to_string());
    };
    let url = url.trim_end_matches('/');
    if url.is_empty() {
        return Ok(cloud_base.to_string());
    }
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(url.to_string())
    } else {
        Err(LoginFailure::InvalidOrganizationServer)
    }
}

impl From<SessionError> for LoginFailure {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::InvalidCredentials => LoginFailure::InvalidCredentials,
            SessionError::Network(_) => LoginFailure::Network,
            SessionError::UnexpectedResponse(_) | SessionError::Storage(_) => LoginFailure::Server,
        }
    }
}

pub struct SlintComposition {
    api_origin: String,
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
        // Separate keyring service from the Tauri shell so both can be installed on
        // one device without overwriting each other's session (decision-log).
        let storage: Arc<dyn SecureStorage> = Arc::new(
            KeyringSecureStorage::for_service("com.onyx.platform.slint")
                .map_err(|e| format!("credential store: {e}"))?,
        );
        Self::open_with(OpenOptions {
            data_dir,
            storage,
            api_origin: ONYX_API_ORIGIN.to_string(),
        })
        .await
    }

    /// Opens the composition with explicit data folder, credential store, and
    /// API origin. `open` uses the device defaults; tests use temporary folders,
    /// an in-memory store, and an unreachable origin.
    pub async fn open_with(options: OpenOptions) -> Result<Arc<Self>, String> {
        let OpenOptions {
            data_dir,
            storage,
            api_origin,
        } = options;
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

        let session = SessionAuthority::with_api_factory(
            &api_origin,
            http_api_factory(),
            Arc::clone(&storage),
        );
        let authority: Arc<dyn AuthorityProvider> =
            Arc::new(SessionBearerProvider::new(Arc::clone(&session)));
        let socket_factory = Arc::new(TungsteniteRelaySocketFactory::with_cancellation(
            replica,
            session.connection_cancellation_source(),
        ));
        let relay = Arc::new(CloudRelayTransport::new(
            &api_origin,
            authority,
            socket_factory,
        ));

        let composition = Arc::new(Self {
            api_origin,
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
                    self.start_outbox(
                        &summary.organization_id.to_string(),
                        &summary.server_address,
                    )
                    .await;
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %e, "stored session could not be read"),
        }
    }

    /// Asks the cloud registry which server serves the organization. A null
    /// `server_url` means the cloud server itself. Only http(s) is accepted.
    async fn resolve_organization_server(&self, organization: &str) -> Result<String, LoginFailure> {
        let identifier = organization.trim();
        if identifier.is_empty() {
            return Err(LoginFailure::OrganizationNotFound);
        }
        let response = reqwest::Client::new()
            .get(format!("{}/api/organizations/lookup", self.api_origin))
            .query(&[("identifier", identifier)])
            .send()
            .await
            .map_err(|_| LoginFailure::Network)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(LoginFailure::OrganizationNotFound);
        }
        if !response.status().is_success() {
            return Err(LoginFailure::Server);
        }
        let body = response.text().await.map_err(|_| LoginFailure::Server)?;
        server_from_lookup(&body, &self.api_origin)
    }

    /// Signs in with the source-confirmed three fields. The organization
    /// determines the server, and the resulting session is pinned to it.
    pub async fn login(
        &self,
        organization: &str,
        username: &str,
        password: &str,
    ) -> Result<SessionSummary, LoginFailure> {
        let server = self.resolve_organization_server(organization).await?;
        let outcome = self
            .session
            .sign_in_at(&server, Some(organization.trim()), username, password)
            .await?;
        if !outcome.persisted {
            tracing::warn!("session is active in memory only; keyring save failed");
        }
        let summary = outcome.summary;
        self.start_outbox(&summary.organization_id.to_string(), &summary.server_address)
            .await;
        Ok(summary)
    }

    /// The live session, if any. Used at startup so a restored session is not
    /// hidden behind the login pane.
    pub async fn current_summary(&self) -> Option<SessionSummary> {
        self.session.current_summary().await
    }

    /// Changes the account password on the session's own server. The Worker
    /// revokes every session on success, so the local session is signed out too.
    pub async fn change_password(
        &self,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), PasswordFailure> {
        let summary = self
            .session
            .current_summary()
            .await
            .ok_or(PasswordFailure::SessionExpired)?;
        let token = self
            .session
            .current_access_token()
            .await
            .ok_or(PasswordFailure::SessionExpired)?;
        let response = reqwest::Client::new()
            .post(format!("{}/api/auth/password", summary.server_address))
            .bearer_auth(token)
            .json(&serde_json::json!({
                "current_password": current_password,
                "new_password": new_password,
            }))
            .send()
            .await
            .map_err(|_| PasswordFailure::Network)?;
        if response.status().is_success() {
            self.logout().await;
            return Ok(());
        }
        let body = response.text().await.unwrap_or_default();
        let code = serde_json::from_str::<Value>(&body)
            .ok()
            .and_then(|value| {
                value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            })
            .unwrap_or_default();
        Err(password_failure_code(&code))
    }

    pub async fn logout(&self) {
        self.stop_outbox().await;
        self.session.sign_out().await;
    }

    /// Builds the app state for `organization_id` and starts its sync agent
    /// against the server that issued the session. A failure is logged; login
    /// still succeeds and rows stay in the local outbox until a later attempt.
    async fn start_outbox(&self, organization_id: &str, server_address: &str) {
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
            cloud_relay_endpoint: relay_ws_endpoint(server_address),
            local_discovery: None,
            cloud_relay_auth_provider: Arc::new(SessionBearerProvider::new(Arc::clone(
                &self.session,
            ))),
            cloud_relay_socket_factory: Arc::new(TungsteniteRelaySocketFactory::with_cancellation(
                self.replica,
                self.session.connection_cancellation_source(),
            )),
            blob_store_root: self.data_dir.join("blobs"),
            owner_authority: None,
            cloud_outbox_endpoint: Some(format!("{server_address}/api/sync/outbox")),
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

/// Inputs for `SlintComposition::open_with`.
pub struct OpenOptions {
    pub data_dir: std::path::PathBuf,
    pub storage: Arc<dyn SecureStorage>,
    pub api_origin: String,
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    use desktop_session::session::{self, StoredSession};
    use keyring_core::mock::Store as MockStore;
    use platform_kernel::{ObjectId, OrganizationId};

    /// Unreachable origin: refresh fails fast with a network error, not a verdict.
    const OFFLINE_ORIGIN: &str = "http://127.0.0.1:9";

    fn options(dir: &std::path::Path, storage: Arc<dyn SecureStorage>) -> OpenOptions {
        OpenOptions {
            data_dir: dir.to_path_buf(),
            storage,
            api_origin: OFFLINE_ORIGIN.to_string(),
        }
    }

    fn mock_storage() -> Arc<dyn SecureStorage> {
        let store = MockStore::new().expect("mock store");
        Arc::new(KeyringSecureStorage::with_store(
            "com.onyx.platform.slint.test",
            store,
        ))
    }

    #[tokio::test]
    async fn fresh_start_opens_signed_out_with_stable_replica() {
        let dir = tempfile::tempdir().unwrap();
        let storage = mock_storage();

        let first = SlintComposition::open_with(options(dir.path(), Arc::clone(&storage)))
            .await
            .expect("first open");
        assert!(first.session.current_summary().await.is_none());
        let replica = first.replica;
        drop(first);

        // Restart over the same folder: migrations are idempotent and the
        // replica identity persists.
        let second = SlintComposition::open_with(options(dir.path(), storage))
            .await
            .expect("second open");
        assert_eq!(second.replica, replica);
        assert!(second.session.current_summary().await.is_none());
    }

    #[tokio::test]
    async fn offline_restart_keeps_stored_session_record() {
        let dir = tempfile::tempdir().unwrap();
        let storage = mock_storage();
        let stored = StoredSession {
            server_address: OFFLINE_ORIGIN.to_string(),
            refresh_token: "refresh-token-for-offline-test".to_string(),
            user_id: ObjectId::new_random(),
            organization_id: OrganizationId::new_random(),
            username: "tester".to_string(),
            is_admin: false,
            must_change_password: false,
        };
        session::save(&storage, &stored).await.expect("save record");

        let composition = SlintComposition::open_with(options(dir.path(), Arc::clone(&storage)))
            .await
            .expect("open offline");

        // Network failure is not a verdict: the app starts signed out, but the
        // stored record survives for the next launch.
        assert!(composition.session.current_summary().await.is_none());
        let still_stored = session::load(&storage)
            .await
            .expect("load")
            .expect("record kept");
        assert_eq!(still_stored.username, "tester");
    }

    #[tokio::test]
    async fn logout_without_session_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let composition = SlintComposition::open_with(options(dir.path(), mock_storage()))
            .await
            .expect("open");
        composition.logout().await;
        composition.logout().await;
        assert!(composition.session.current_summary().await.is_none());
    }

    #[test]
    fn relay_endpoints_follow_the_session_server() {
        assert_eq!(relay_ws_endpoint("https://org.example"), "wss://org.example");
        assert_eq!(relay_ws_endpoint("http://127.0.0.1:9"), "ws://127.0.0.1:9");
        assert_eq!(relay_ws_endpoint("ws://already"), "ws://already");
    }

    #[test]
    fn password_error_codes_are_mapped_by_identifier_only() {
        assert_eq!(
            password_failure_code("CURRENT_PASSWORD_INCORRECT"),
            PasswordFailure::CurrentPasswordIncorrect
        );
        assert_eq!(password_failure_code("WEAK_PASSWORD"), PasswordFailure::WeakPassword);
        assert_eq!(
            password_failure_code("PASSWORD_UNCHANGED"),
            PasswordFailure::PasswordUnchanged
        );
        assert_eq!(password_failure_code("something else"), PasswordFailure::Server);
    }

    #[test]
    fn lookup_server_is_display_only_derived_from_registry_response() {
        assert_eq!(
            server_from_lookup(r#"{"server_url":null}"#, "https://cloud.example").unwrap(),
            "https://cloud.example"
        );
        assert_eq!(
            server_from_lookup(r#"{"server_url":"https://org.example/"}"#, "https://cloud.example")
                .unwrap(),
            "https://org.example"
        );
        assert_eq!(
            server_from_lookup(
                r#"{"error":"INVALID_ORGANIZATION"}"#,
                "https://cloud.example"
            )
            .unwrap(),
            "https://cloud.example"
        );
        assert_eq!(
            server_from_lookup(r#"{"server_url":"ftp://org.example"}"#, "https://cloud.example"),
            Err(LoginFailure::InvalidOrganizationServer)
        );
    }
}

#[cfg(test)]
mod desktop_login_tests {
    use super::*;
    use axum::{
        extract::{Json, State},
        http::StatusCode,
        routing::{get, post},
        Router,
    };
    use serde_json::json;
    use std::sync::Mutex as StdMutex;

    #[derive(Default)]
    struct ServerMock {
        lookup_status: StdMutex<u16>,
        login_status: StdMutex<u16>,
        password_status: StdMutex<u16>,
        login_body: StdMutex<Option<serde_json::Value>>,
        password_body: StdMutex<Option<serde_json::Value>>,
    }

    async fn lookup(State(mock): State<Arc<ServerMock>>) -> (StatusCode, Json<serde_json::Value>) {
        let status = *mock.lookup_status.lock().unwrap();
        (StatusCode::from_u16(status).unwrap(), Json(json!({"server_url": null})))
    }

    async fn login(
        State(mock): State<Arc<ServerMock>>,
        Json(body): Json<serde_json::Value>,
    ) -> (StatusCode, Json<serde_json::Value>) {
        *mock.login_body.lock().unwrap() = Some(body);
        let status = *mock.login_status.lock().unwrap();
        (
            StatusCode::from_u16(status).unwrap(),
            Json(json!({
                "access_token": "access-token",
                "refresh_token": "refresh-token",
                "expires_in": 3600,
                "user": {
                    "id": uuid::Uuid::from_bytes([1; 16]).to_string(),
                    "username": "tester",
                    "organization_id": uuid::Uuid::from_bytes([2; 16]).to_string(),
                    "is_admin": false,
                    "must_change_password": true
                }
            })),
        )
    }

    async fn logout() -> (StatusCode, Json<serde_json::Value>) {
        (StatusCode::OK, Json(json!({"success": true})))
    }

    async fn password(
        State(mock): State<Arc<ServerMock>>,
        Json(body): Json<serde_json::Value>,
    ) -> (StatusCode, Json<serde_json::Value>) {
        *mock.password_body.lock().unwrap() = Some(body);
        let status = *mock.password_status.lock().unwrap();
        (StatusCode::from_u16(status).unwrap(), Json(json!({"success": true})))
    }

    async fn serve(mock: Arc<ServerMock>) -> String {
        let app = Router::new()
            .route("/api/organizations/lookup", get(lookup))
            .route("/api/auth/login", post(login))
            .route("/api/auth/logout", post(logout))
            .route("/api/auth/password", post(password))
            .with_state(mock);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        format!("http://{address}")
    }

    fn storage() -> Arc<dyn SecureStorage> {
        let store = keyring_core::mock::Store::new().expect("mock store");
        Arc::new(KeyringSecureStorage::with_store(
            "com.onyx.platform.slint.test",
            store,
        ))
    }

    async fn open(origin: &str) -> (Arc<SlintComposition>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let composition = SlintComposition::open_with(OpenOptions {
            data_dir: dir.path().to_path_buf(),
            storage: storage(),
            api_origin: origin.to_string(),
        })
        .await
        .expect("open");
        (composition, dir)
    }

    #[tokio::test]
    async fn three_field_login_uses_the_registry_and_the_issuing_server() {
        let mock = Arc::new(ServerMock::default());
        *mock.lookup_status.lock().unwrap() = 200;
        *mock.login_status.lock().unwrap() = 200;
        let origin = serve(mock.clone()).await;
        let (composition, _dir) = open(&origin).await;

        let summary = composition
            .login("Acme", "tester", "password")
            .await
            .expect("login");
        assert_eq!(summary.server_address, origin);
        assert!(summary.must_change_password);
        let body = mock.login_body.lock().unwrap().clone().expect("login body");
        assert_eq!(body.get("organization").and_then(|v| v.as_str()), Some("Acme"));
        assert_eq!(body.get("username").and_then(|v| v.as_str()), Some("tester"));
    }

    #[tokio::test]
    async fn unknown_organization_is_rejected_before_login() {
        let mock = Arc::new(ServerMock::default());
        *mock.lookup_status.lock().unwrap() = 404;
        let origin = serve(mock.clone()).await;
        let (composition, _dir) = open(&origin).await;

        let error = composition
            .login("Missing", "tester", "password")
            .await
            .expect_err("unknown organization");
        assert_eq!(error, LoginFailure::OrganizationNotFound);
        assert!(mock.login_body.lock().unwrap().is_none());
    }

    #[tokio::test]
    async fn password_change_uses_the_session_server_and_signs_out() {
        let mock = Arc::new(ServerMock::default());
        *mock.lookup_status.lock().unwrap() = 200;
        *mock.login_status.lock().unwrap() = 200;
        *mock.password_status.lock().unwrap() = 200;
        let origin = serve(mock.clone()).await;
        let (composition, _dir) = open(&origin).await;
        composition
            .login("Acme", "tester", "password")
            .await
            .expect("login");

        composition
            .change_password("old-password", "new-password-12")
            .await
            .expect("password change");
        let body = mock.password_body.lock().unwrap().clone().expect("password body");
        assert_eq!(body.get("current_password").and_then(|v| v.as_str()), Some("old-password"));
        assert_eq!(body.get("new_password").and_then(|v| v.as_str()), Some("new-password-12"));
        assert!(composition.current_summary().await.is_none());
    }
}
