//! `desktop-shell` — the Tauri desktop client, a thin wrapper around
//! `client-composition`'s `AppState` (Team Prompt 5 §4.1; C1 in
//! `DECISIONS.md`).
//!
//! # Provenance
//! Built against Tauri **2.x** (T2, `DECISIONS.md` — the Team Prompt's
//! `^1.5` pin is stale; verified against the real, current Tauri 2 docs,
//! not assumed). Notably, Tauri 2 moves commands into `lib.rs` (not
//! `main.rs`, as Tauri 1 and Team Prompt 5's own snippet do) and
//! requires `#[cfg_attr(mobile, tauri::mobile_entry_point)] pub fn run()`
//! as the entry point.

mod relay_socket;
mod secure_storage;

use std::sync::Arc;

use client_composition::app_state::{AppState, AppStateConfig};
use client_composition::event_bus::EventFilter;
use client_composition::query_registry::QueryEnvelope;
use platform_contracts::CommandEnvelope;
use platform_kernel::{ObjectId, OrganizationId, ReplicaId};
use secure_storage::keyring_adapter::KeyringSecureStorage;
use secure_storage::SecureStorage;
use sqlx::sqlite::SqlitePoolOptions;
use tauri::{AppHandle, Emitter, Manager};

/// A Tauri command's error type must implement `serde::Serialize`
/// (verified against the real Tauri 2 command docs before writing this —
/// see the "Error Handling" section fetched from v2.tauri.app). Wraps
/// every error this shell's commands can produce into one serializable
/// shape rather than leaking `client-composition`'s internal error enums
/// (which do not derive `Serialize`) across the IPC boundary.
#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", content = "message")]
#[serde(rename_all = "camelCase")]
enum ShellError {
    Command(String),
    Query(String),
    Storage(String),
    InvalidArgument(String),
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ShellError {}

/// Executes a command envelope (JSON, per Team Prompt 5 §3.2's
/// `execute_command`) through `AppState`'s real `CommandRegistry`.
#[tauri::command]
async fn execute_command(
    state: tauri::State<'_, Arc<AppState>>,
    envelope: CommandEnvelope<serde_json::Value>,
) -> Result<serde_json::Value, ShellError> {
    state
        .command_registry
        .dispatch(envelope)
        .await
        .map_err(|e| ShellError::Command(e.to_string()))
}

/// Executes a query (per Team Prompt 5 §3.2's `execute_query`) through
/// `AppState`'s real `QueryRegistry`.
#[tauri::command]
async fn execute_query(
    state: tauri::State<'_, Arc<AppState>>,
    query_type: String,
    target_id: ObjectId,
) -> Result<serde_json::Value, ShellError> {
    state
        .query_registry
        .dispatch(QueryEnvelope {
            query_type,
            target_id,
        })
        .await
        .map_err(|e| ShellError::Query(e.to_string()))
}

/// Subscribes to the event bus and forwards every matching event to the
/// webview as a `"onyx:event"` Tauri event (`Emitter::emit`, the real
/// Tauri 2 API — verified before writing this; Tauri 1's `emit_all` does
/// not exist in Tauri 2). Runs for the lifetime of the app process; per
/// Team Prompt 5 §3.2's `subscribe_events -> EventStreamId`, returns the
/// stream id so the frontend has something to reference, even though
/// there is currently no matching `unsubscribe_events` command to pair
/// it with (flagged, not silently assumed complete — Team Prompt 5 does
/// not specify one either).
#[tauri::command]
async fn subscribe_events(
    app: AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    organization_id: OrganizationId,
) -> Result<String, ShellError> {
    let mut subscription = state.event_bus.subscribe(EventFilter {
        organization_id,
        event_types: None,
    });
    let stream_id = subscription.id;

    tauri::async_runtime::spawn(async move {
        while let Some(event) = subscription.recv().await {
            if let Err(e) = app.emit("onyx:event", event) {
                tracing::warn!(error = %e, "failed to emit onyx:event to webview");
            }
        }
    });

    Ok(format!("{:?}", stream_id.0))
}

/// Returns the sync agent's current status (Team Prompt 5 §3.2's
/// `get_sync_status`).
#[tauri::command]
async fn get_sync_status(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, ShellError> {
    let status = state.sync_agent.status().await;
    serde_json::to_value(&status).map_err(|e| ShellError::Command(e.to_string()))
}

/// Stores a secret via the platform `SecureStorage` adapter (Team Prompt
/// 5 §3.1). `value` is accepted as a UTF-8 string over IPC — the
/// underlying port stores raw bytes (`&[u8]`), so this command narrows
/// to text secrets specifically (tokens, keys — the frozen contract's own
/// examples, `"auth.refresh_token"`/`"sync.sync_key"`, are both textual).
#[tauri::command]
async fn store_secret(
    storage: tauri::State<'_, Arc<dyn SecureStorage>>,
    key: String,
    value: String,
) -> Result<(), ShellError> {
    storage
        .store_secret(&key, value.as_bytes())
        .await
        .map_err(|e| ShellError::Storage(e.to_string()))
}

/// Retrieves a secret; `None` (serialized as `null`) if not found.
#[tauri::command]
async fn get_secret(
    storage: tauri::State<'_, Arc<dyn SecureStorage>>,
    key: String,
) -> Result<Option<String>, ShellError> {
    let bytes = storage
        .get_secret(&key)
        .await
        .map_err(|e| ShellError::Storage(e.to_string()))?;
    match bytes {
        None => Ok(None),
        Some(b) => String::from_utf8(b).map(Some).map_err(|e| {
            ShellError::InvalidArgument(format!("stored secret was not valid UTF-8: {e}"))
        }),
    }
}

#[tauri::command]
async fn delete_secret(
    storage: tauri::State<'_, Arc<dyn SecureStorage>>,
    key: String,
) -> Result<(), ShellError> {
    storage
        .delete_secret(&key)
        .await
        .map_err(|e| ShellError::Storage(e.to_string()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt::try_init().ok();

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            let data_dir = app_handle
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to resolve app data directory: {e}"))?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("onyx.sqlite");

            // AppState::new needs an already-connected pool; building one
            // requires async I/O, which `setup` (a sync closure) cannot
            // directly await. Bridged via `tauri::async_runtime::block_on`
            // — the documented, supported way to run async setup work
            // inside `setup`'s synchronous closure.
            let app_handle_for_state = app_handle.clone();
            tauri::async_runtime::block_on(async move {
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect(&format!("sqlite://{}?mode=rwc", db_path.display()))
                    .await
                    .expect("failed to open onyx.sqlite");

                let schema = include_str!(
                    "../../../../migrations/sqlite/20260101000000_initial_schema.up.sql"
                );
                for stmt in schema.split(';') {
                    let stmt = stmt.trim();
                    if stmt.is_empty() {
                        continue;
                    }
                    // Migrations may already be applied on a second
                    // launch; CREATE TABLE/INDEX statements in the real
                    // schema use no IF NOT EXISTS guard (see the schema
                    // file itself), so a second run's failures here are
                    // expected on all but the first launch and are
                    // intentionally not treated as fatal — only the
                    // first `connect`/pool-open failure above is.
                    let _ = sqlx::query(stmt).execute(&pool).await;
                }

                let storage: Arc<dyn SecureStorage> = Arc::new(KeyringSecureStorage::new());

                // Generated once here and shared with the relay socket
                // factory below, which must announce the same id to the relay
                // for this replica to be addressable.
                let local_replica = ReplicaId::new_random();

                let config = AppStateConfig {
                    local_replica,
                    // TODO(auth/org resolution): organization_id should
                    // come from the authenticated user's session, not be
                    // randomly generated per launch. No login/auth flow
                    // exists yet in this increment's scope (Increment 7)
                    // — flagged, not silently assumed resolved.
                    organization_id: OrganizationId::new_random(),
                    sync_agent_config: client_composition::sync_agent::SyncAgentConfig::default(),
                    event_bus_capacity: 1024,
                    // TODO(cloud relay): no real endpoint/auth/socket
                    // implementation exists yet (see DECISIONS.md's
                    // AppState entry) — these are placeholders so the
                    // app starts; Cloud Relay sync will fail at connect
                    // time until real ones are supplied.
                    cloud_relay_endpoint: "wss://relay.onyx.example/v1".to_string(),
                    cloud_relay_auth_provider: Arc::new(
                        sync_transport::placeholder_types::StaticAuthorityProvider(String::new()),
                    ),
                    cloud_relay_socket_factory: Arc::new(
                        relay_socket::TungsteniteRelaySocketFactory::new(local_replica),
                    ),
                };

                let state = Arc::new(AppState::new(pool, config));
                app_handle_for_state.manage(state);
                app_handle_for_state.manage(storage);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            execute_command,
            execute_query,
            subscribe_events,
            get_sync_status,
            store_secret,
            get_secret,
            delete_secret,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// `NotYetImplementedSocketFactory` lived here and returned
// `TransportError::Unreachable` for every call. It is replaced by
// `relay_socket::TungsteniteRelaySocketFactory`, which opens a real WebSocket
// to the relay endpoint now served at `/api/relay/:target_id`.
