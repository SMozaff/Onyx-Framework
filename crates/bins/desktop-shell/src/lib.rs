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
//!
//! # Real login (2026-08-19)
//! Previously `AppState` was constructed once in `setup()` with
//! `OrganizationId::new_random()` — a `TODO(auth/org resolution)`
//! comment at that call site flagged that no login flow existed yet and
//! that this meant every launch could target a different, orphaned
//! organization. See `session.rs`'s module doc for the fix design. The
//! key structural change here: managed `AppState` is now
//! `Arc<tokio::sync::RwLock<Arc<AppState>>>` instead of a bare
//! `Arc<AppState>`, so the `login` command can build a *new* `AppState`
//! (with the real, server-issued `organization_id`) and atomically swap
//! it in — without restarting the Tauri process. Tauri's own state
//! management docs confirm this is the supported pattern for state that
//! must change after `setup()` (managed state itself cannot be
//! replaced, but interior mutability via a lock can be swapped) —
//! verified against v2.tauri.app before choosing this over a
//! process-restart approach.

use relay_websocket_adapter as relay_socket;
mod secure_storage;

use std::sync::Arc;

use desktop_session::session;
use desktop_session::session_authority::{SessionAuthority, SessionBearerProvider, SessionSummary};

/// Canonical ONYX Cloud API origin. Distributed clients must not accept
/// endpoint overrides.
const CLOUD_API_BASE: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

/// Keys owned by the session authority. The generic secret commands below
/// refuse them, so the webview cannot read or overwrite the stored refresh
/// token.
const RESERVED_SECRET_PREFIX: &str = "auth.";

fn reject_reserved_secret_key(key: &str) -> Result<(), ShellError> {
    if key.starts_with(RESERVED_SECRET_PREFIX) {
        return Err(ShellError::InvalidArgument(
            "reserved secret key".to_string(),
        ));
    }
    Ok(())
}

use client_composition::app_state::{AppState, AppStateConfig};
use client_composition::event_bus::EventFilter;
use client_composition::query_registry::QueryEnvelope;
use platform_contracts::CommandEnvelope;
use platform_kernel::{ObjectId, OrganizationId, ReplicaId};
use secure_storage::keyring_adapter::KeyringSecureStorage;
use secure_storage::SecureStorage;
use sqlx::sqlite::SqlitePoolOptions;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::RwLock;

/// Managed state's real type. Wrapped so `login`/`logout` can replace
/// the inner `Arc<AppState>` after construction — see module doc.
type SharedAppState = Arc<RwLock<Arc<AppState>>>;

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
    Auth(String),
}

impl std::fmt::Display for ShellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ShellError {}

impl From<session::SessionError> for ShellError {
    fn from(e: session::SessionError) -> Self {
        match e {
            session::SessionError::InvalidCredentials => {
                ShellError::Auth("Invalid username or password".to_string())
            }
            session::SessionError::Network(msg) => {
                ShellError::Auth(format!("Could not reach the server: {msg}"))
            }
            other => ShellError::Auth(other.to_string()),
        }
    }
}

/// Executes a command envelope (JSON, per Team Prompt 5 §3.2's
/// `execute_command`) through `AppState`'s real `CommandRegistry`.
#[tauri::command]
async fn execute_command(
    state: tauri::State<'_, SharedAppState>,
    envelope: CommandEnvelope<serde_json::Value>,
) -> Result<serde_json::Value, ShellError> {
    let app_state = state.read().await.clone();
    app_state
        .command_registry
        .dispatch(envelope)
        .await
        .map_err(|e| ShellError::Command(e.to_string()))
}

/// Executes a query (per Team Prompt 5 §3.2's `execute_query`) through
/// `AppState`'s real `QueryRegistry`.
#[tauri::command]
async fn execute_query(
    state: tauri::State<'_, SharedAppState>,
    query_type: String,
    target_id: ObjectId,
) -> Result<serde_json::Value, ShellError> {
    let app_state = state.read().await.clone();
    app_state
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
///
/// Reads `AppState` once at subscribe time — a subscription started
/// before a `login` swap will keep listening on the pre-login event bus
/// after the swap. Acceptable for now: the frontend re-subscribes after
/// a successful login, so this is a short-lived window, not a permanent
/// leak, but is flagged here rather than silently assumed correct across
/// a login swap.
#[tauri::command]
async fn subscribe_events(
    app: AppHandle,
    state: tauri::State<'_, SharedAppState>,
    organization_id: OrganizationId,
) -> Result<String, ShellError> {
    let app_state = state.read().await.clone();
    let mut subscription = app_state.event_bus.subscribe(EventFilter {
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
    state: tauri::State<'_, SharedAppState>,
) -> Result<serde_json::Value, ShellError> {
    let app_state = state.read().await.clone();
    let status = app_state.sync_agent.status().await;
    serde_json::to_value(&status).map_err(|e| ShellError::Command(e.to_string()))
}

/// Uploads a file from the local filesystem. Phase 1 (Desktop & Web
/// Completion) addition — see `client_composition::file_upload`'s module
/// doc comment for why this is a dedicated command rather than another
/// `execute_command` command_type.
///
/// Takes a filesystem **path** rather than the file's bytes: the webview
/// would otherwise have to read the file into JS memory and ship it
/// through Tauri's IPC (which JSON-encodes its arguments), costing a full
/// extra copy plus base64-ish inflation for what can be up to a 100 MB
/// file (`file_domain::value::MAX_FILE_SIZE_BYTES`). Reading it here, in
/// Rust, on the native side avoids that entirely.
#[tauri::command]
async fn upload_file(
    state: tauri::State<'_, SharedAppState>,
    path: String,
    organization_id: OrganizationId,
    user_id: ObjectId,
    device_id: ObjectId,
) -> Result<serde_json::Value, ShellError> {
    let content = tokio::fs::read(&path)
        .await
        .map_err(|e| ShellError::InvalidArgument(format!("reading {path}: {e}")))?;

    let file_name = std::path::Path::new(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("untitled")
        .to_string();

    // No MIME sniffing library is a dependency of this workspace, and
    // guessing a type from the extension alone would be a half-measure
    // that silently mislabels files. `file_domain::value::MimeType` only
    // validates non-emptiness (see its own doc comment), so a correct,
    // honest default is the generic binary type; a real content-type
    // detector is a follow-up, flagged rather than faked here.
    let mime_type = "application/octet-stream".to_string();

    let actor = platform_kernel::ActorContext {
        user_id,
        device_id,
        organization_id,
    };

    let app_state = state.read().await.clone();
    let outcome = app_state
        .file_upload_coordinator
        .upload_new_file(actor, file_name, mime_type, &content)
        .await
        .map_err(|e| ShellError::Command(e.to_string()))?;

    serde_json::to_value(&outcome).map_err(|e| ShellError::Command(e.to_string()))
}

/// Downloads a previously-uploaded file's content by its content hash,
/// writing it to `destination_path`. Returns the number of bytes
/// written, or an `InvalidArgument` error if no blob is stored for that
/// hash (e.g. the `FileAsset` was created through a path that never
/// uploaded real content).
#[tauri::command]
async fn download_file(
    state: tauri::State<'_, SharedAppState>,
    content_hash: String,
    destination_path: String,
) -> Result<u64, ShellError> {
    let app_state = state.read().await.clone();
    let content = app_state
        .file_upload_coordinator
        .download(&content_hash)
        .await
        .map_err(|e| ShellError::Command(e.to_string()))?
        .ok_or_else(|| {
            ShellError::InvalidArgument(format!("no stored content for hash {content_hash}"))
        })?;

    tokio::fs::write(&destination_path, &content)
        .await
        .map_err(|e| ShellError::Storage(format!("writing {destination_path}: {e}")))?;

    Ok(content.len() as u64)
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
    reject_reserved_secret_key(&key)?;
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
    reject_reserved_secret_key(&key)?;
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
    reject_reserved_secret_key(&key)?;
    storage
        .delete_secret(&key)
        .await
        .map_err(|e| ShellError::Storage(e.to_string()))
}

/// What the frontend receives after a successful login (or from
/// `get_current_session` on startup). Carries no tokens: the frontend never
/// needs them, so they never enter a webview context.
///
/// `persistence_notice` is set when the keyring could not store the session.
/// The session still works for this launch, and the message is safe to show.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionInfo {
    username: String,
    is_admin: bool,
    server_address: String,
    user_id: String,
    organization_id: String,
    device_id: String,
    persistence_notice: Option<String>,
}

impl SessionInfo {
    fn from_summary(
        summary: &SessionSummary,
        local_replica: ReplicaId,
        persistence_notice: Option<String>,
    ) -> Self {
        Self {
            username: summary.username.clone(),
            is_admin: summary.is_admin,
            server_address: summary.server_address.clone(),
            user_id: summary.user_id.to_string(),
            organization_id: summary.organization_id.to_string(),
            device_id: uuid::Uuid::from_bytes(local_replica.0).to_string(),
            persistence_notice,
        }
    }
}

/// Returns the currently persisted session, if any — lets the frontend
/// decide on launch whether to show the login screen or go straight to
/// the main app. `None` is the normal, expected state on a fresh
/// install or after `logout`, not an error.
#[tauri::command]
async fn get_current_session(
    authority: tauri::State<'_, Arc<SessionAuthority>>,
    local_replica: tauri::State<'_, ReplicaId>,
) -> Result<Option<SessionInfo>, ShellError> {
    // A stored record alone does not mean signed in. Only a live session,
    // established by a successful restore or sign-in, counts.
    Ok(authority
        .current_summary()
        .await
        .as_ref()
        .map(|summary| SessionInfo::from_summary(summary, *local_replica.inner(), None)))
}

/// Signs in through `SessionAuthority`, and on success:
/// 1. Installs the access token in memory and persists only the refresh
///    token (keyring). The next launch restores the session under the same
///    organization. If the keyring is unavailable, the session lasts for this
///    launch and the response carries a `persistenceNotice`.
/// 2. Rebuilds `AppState` under the real, server-issued
///    `organization_id` (fixing the frontend/backend org-id mismatch bug
///    `useSession.ts` flagged) and atomically swaps it into the shared
///    `RwLock`, so the rest of the running app picks up the real
///    identity without a process restart.
///
/// The device's local SQLite database and `ReplicaId` are NOT reset on
/// login — they are properties of this physical device/install, not of
/// who happens to be logged in, and a second login (e.g. after logout)
/// should not fragment sync history for data already on this machine.
/// The two real input values `login` needs from the caller, grouped
/// into one struct so the command itself stays under Clippy's
/// `too_many_arguments` limit (7) once the five injected Tauri state
/// handles are also counted — confirmed via a real `cargo clippy
/// --workspace -- -D warnings` failure (`8/7`) before this existed, not
/// a speculative refactor. `#[serde(rename_all = "camelCase")]` so the
/// frontend's `invoke("login", { credentials: { username, password } })` call
/// deserializes using the same camelCase convention as other Tauri commands.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginRequest {
    username: String,
    password: String,
}

#[tauri::command]
async fn login(
    app: AppHandle,
    state: tauri::State<'_, SharedAppState>,
    authority: tauri::State<'_, Arc<SessionAuthority>>,
    local_replica: tauri::State<'_, ReplicaId>,
    hierarchy_cache: tauri::State<'_, client_composition::hierarchy_cache::HierarchyCache>,
    credentials: LoginRequest,
) -> Result<SessionInfo, ShellError> {
    let LoginRequest { username, password } = credentials;
    let outcome = authority.sign_in(&username, &password).await?;

    let new_state = build_app_state(
        &app,
        outcome.summary.organization_id,
        Some(&outcome.summary),
    )
    .await?;

    *state.write().await = new_state;

    // Best-effort, same reasoning as the startup-resume case in
    // setup(): a transient failure here must not lock the person out of
    // logging in. Logged, not propagated.
    if let Some(access_token) = authority.current_access_token().await {
        if let Err(e) = hierarchy_cache.refresh(CLOUD_API_BASE, &access_token).await {
            tracing::warn!(error = %e, "failed to refresh approval-authority cache after login");
        }
    }

    let notice = (!outcome.persisted).then(|| {
        "Your secure keyring is unavailable. You stay signed in until this app closes, then you will need to sign in again."
            .to_string()
    });
    Ok(SessionInfo::from_summary(
        &outcome.summary,
        *local_replica.inner(),
        notice,
    ))
}

/// Clears the persisted session and rebuilds `AppState` under a fresh
/// random organization — the same "locked out, pre-login" placeholder
/// state a brand-new install starts in. The local SQLite database and
/// `ReplicaId` are left untouched, matching `login`'s own rationale:
/// they belong to the device, not the session.
#[tauri::command]
async fn logout(
    app: AppHandle,
    state: tauri::State<'_, SharedAppState>,
    authority: tauri::State<'_, Arc<SessionAuthority>>,
    hierarchy_cache: tauri::State<'_, client_composition::hierarchy_cache::HierarchyCache>,
) -> Result<(), ShellError> {
    // Clears the in-memory pair and the keyring record, then revokes the
    // latest pair server-side. See session_authority::sign_out.
    authority.sign_out().await;
    let new_state = build_app_state(&app, OrganizationId::new_random(), None).await?;
    *state.write().await = new_state;
    // Defensive: a different person logging in next on this same
    // device must not inherit the previous person's cached hierarchy
    // even transiently. `login` always overwrites this cache anyway,
    // but clearing here removes the window where it wouldn't (an
    // approval attempted between logout and the next login's own
    // refresh completing).
    hierarchy_cache.clear().await;
    Ok(())
}

/// Builds a fresh `AppState` bound to `organization_id`, reusing this
/// device's already-open SQLite pool (fetched from Tauri's managed
/// state — see `setup()`) rather than reopening it, since the pool
/// itself has nothing to do with which organization is logged in.
///
/// `session` is `Some` for a real login (wires the real access token
/// into `cloud_relay_auth_provider` and derives the sync relay endpoint
/// from `server_address`) or `None` for the pre-login/logged-out
/// placeholder (keeps the previous `ONYX_RELAY_ENDPOINT` env var /
/// loopback-default behavior, and the previous test-only
/// `StaticAuthorityProvider(String::new())`, unchanged from before this
/// change for that case).
async fn build_app_state(
    app: &AppHandle,
    organization_id: OrganizationId,
    session: Option<&SessionSummary>,
) -> Result<Arc<AppState>, ShellError> {
    let authority = app.state::<Arc<SessionAuthority>>().inner().clone();
    let pool = app.state::<sqlx::SqlitePool>().inner().clone();
    let local_replica = *app.state::<ReplicaId>().inner();
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| ShellError::Storage(format!("failed to resolve app data directory: {e}")))?;
    let blob_store_root = data_dir.join("blobs");

    let lan_discovery: Option<Arc<dyn sync_transport::Discovery>> =
        match lan_discovery::LanDiscovery::start(
            local_replica,
            organization_id,
            3000,
            hostname_or_none(),
        )
        .await
        {
            Ok(d) => Some(Arc::new(d)),
            Err(e) => {
                tracing::warn!(error = %e, "LAN discovery unavailable; relay only");
                None
            }
        };

    // Derive the ws(s):// relay endpoint from the logged-in server's
    // http(s):// address, so there is exactly one address the person
    // ever configures (the same one entered at login / in Settings) —
    // not a second, independently-set ONYX_RELAY_ENDPOINT that could
    // silently drift out of sync with it. Falls back to the previous
    // env-var/loopback behavior when there is no session yet (pre-login
    // placeholder state).
    let (cloud_relay_endpoint, cloud_relay_auth_provider): (
        String,
        Arc<dyn sync_transport::placeholder_types::AuthorityProvider>,
    ) = match session {
        Some(s) => (
            relay_ws_endpoint_from_http(&s.server_address),
            Arc::new(SessionBearerProvider::new(Arc::clone(&authority))),
        ),
        // Logged out: the same session-backed provider. It has no token, so
        // every call returns NoCredentials (fail closed).
        None => (
            std::env::var("ONYX_RELAY_ENDPOINT")
                .unwrap_or_else(|_| "ws://127.0.0.1:3000".to_string()),
            Arc::new(SessionBearerProvider::new(Arc::clone(&authority))),
        ),
    };

    let config = AppStateConfig {
        local_replica,
        organization_id,
        sync_agent_config: client_composition::sync_agent::SyncAgentConfig::default(),
        event_bus_capacity: 1024,
        cloud_relay_endpoint,
        local_discovery: lan_discovery,
        cloud_relay_auth_provider,
        // Logout and session replacement cancel in-flight relay ticket and
        // handshake attempts through this shared source.
        cloud_relay_socket_factory: Arc::new(
            relay_socket::TungsteniteRelaySocketFactory::with_cancellation(
                local_replica,
                authority.connection_cancellation_source(),
            ),
        ),
        // Tauri shell: outbox upload is not wired (Slint desktop is the target).
        cloud_outbox_endpoint: None,
        blob_store_root,
        // Real HierarchyCache only for an actual logged-in session
        // (mirrors the `cloud_relay_auth_provider` conditional above) —
        // the pre-login/logged-out placeholder AppState has no session
        // to have populated a cache from, so it correctly falls back to
        // client-composition's DenyAllOwnerAuthority (config value
        // `None`) rather than pointing at a cache that's empty anyway.
        owner_authority: session.map(|_| {
            Arc::new(
                app.state::<client_composition::hierarchy_cache::HierarchyCache>()
                    .inner()
                    .clone(),
            ) as Arc<dyn api_server::OwnerAuthority>
        }),
    };

    let state = Arc::new(
        AppState::new(pool, config)
            .await
            .map_err(|e| ShellError::Storage(format!("failed to initialize blob store: {e}")))?,
    );
    let sync_agent = Arc::clone(&state.sync_agent);
    tauri::async_runtime::spawn(async move {
        sync_agent.run().await;
    });

    Ok(state)
}

/// Rewrites an `http(s)://host:port` server address into the matching
/// `ws(s)://host:port` relay endpoint. Falls back to treating the input
/// as already a `ws://`-style address (unchanged) if it doesn't start
/// with a recognized http(s) scheme, rather than silently producing a
/// malformed URL.
fn relay_ws_endpoint_from_http(server_address: &str) -> String {
    if let Some(rest) = server_address.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = server_address.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        server_address.to_string()
    }
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
            // Phase 1 (Desktop & Web Completion): file content lives
            // alongside the SQLite database in the same app data
            // directory. `LocalBlobStore::open` creates this itself if
            // absent, so no `create_dir_all` is needed here.

            // AppState::new needs an already-connected pool; building one
            // requires async I/O, which `setup` (a sync closure) cannot
            // directly await. Bridged via `tauri::async_runtime::block_on`
            // — the documented, supported way to run async setup work
            // inside `setup`'s synchronous closure.
            let app_handle_for_state = app_handle.clone();
            tauri::async_runtime::block_on(async move {
                let options = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&db_path)
                    .create_if_missing(true)
                    // Phase A fix (User Hierarchy) — see the matching
                    // comment in api-server/src/routes/mod.rs for full
                    // rationale: without this, SQLite silently ignores
                    // every foreign key in this schema, including
                    // users.parent_user_id.
                    .foreign_keys(true);
                let pool = SqlitePoolOptions::new()
                    .max_connections(1)
                    .connect_with(options)
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
                // factory (via build_app_state, called below and again by
                // login/logout), which must announce the same id to the
                // relay for this replica to be addressable. Unlike
                // organization_id, this is stable across logins — it
                // identifies this physical device/install, not a session.
                let local_replica = ReplicaId::new_random();

                // Managed ahead of build_app_state so that function can
                // read them back out via app.state:: rather than needing
                // them threaded through as extra parameters on every call
                // (setup's first call, plus every future login/logout).
                app_handle_for_state.manage(pool.clone());
                app_handle_for_state.manage(local_replica);
                app_handle_for_state.manage(storage.clone());
                // Task/Mission approval-authority cache — see
                // client_composition::hierarchy_cache's module doc for
                // the full gap this closes (moved there from a
                // desktop-shell-local module once mobile-core needed the
                // same logic). Registered empty here; populated by `login`
                // (and re-populated on resuming a saved session, below)
                // via a real fetch, never assumed non-empty before that.
                app_handle_for_state.manage(client_composition::hierarchy_cache::HierarchyCache::new());

                // Startup restore. The keyring holds only the refresh token.
                // Restore exchanges it for a new access token, and the
                // rotated refresh token is written back. Any failure leaves
                // the person on the login screen. None of them is fatal.
                let api = Arc::new(session::HttpSessionApi::new(CLOUD_API_BASE)?);
                let authority = desktop_session::session_authority::SessionAuthority::new(
                    CLOUD_API_BASE,
                    api,
                    storage.clone(),
                );
                app_handle_for_state.manage(Arc::clone(&authority));

                let stored = session::load(&storage).await.unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "failed to load saved session, starting signed out");
                    None
                });
                let restored = match stored {
                    Some(stored) => authority.restore(stored).await,
                    None => None,
                };
                let organization_id = restored
                    .as_ref()
                    .map(|s| s.organization_id)
                    .unwrap_or_else(OrganizationId::new_random);

                let state = build_app_state(
                    &app_handle_for_state,
                    organization_id,
                    restored.as_ref(),
                )
                .await?;

                // Populate the approval-authority cache for a restored session.
                // Best-effort: a stale or offline cache degrades to "cannot
                // approve until reachable", and this must not block startup.
                if restored.is_some() {
                    if let Some(access_token) = authority.current_access_token().await {
                        let cache = app_handle_for_state
                            .state::<client_composition::hierarchy_cache::HierarchyCache>();
                        if let Err(e) = cache.refresh(CLOUD_API_BASE, &access_token).await {
                            tracing::warn!(error = %e, "failed to refresh approval-authority cache on startup");
                        }
                    }
                }

                app_handle_for_state.manage::<SharedAppState>(Arc::new(RwLock::new(state)));
                Ok::<(), ShellError>(())
            })?;

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
            upload_file,
            download_file,
            login,
            logout,
            get_current_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// `NotYetImplementedSocketFactory` lived here and returned
// `TransportError::Unreachable` for every call. It is replaced by
// `relay_socket::TungsteniteRelaySocketFactory`, which opens a real WebSocket
// to the relay endpoint now served at `/api/relay/:target_id`.

/// A human-readable name for this device in peer lists. Best-effort: the
/// hostname is a convenience label, not an identifier — `ReplicaId` is.
fn hostname_or_none() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
}

#[cfg(test)]
mod lib_tests {
    use super::{relay_ws_endpoint_from_http, SessionInfo, SessionSummary};
    use platform_kernel::{ObjectId, ReplicaId};

    #[test]
    fn session_info_exposes_non_secret_actor_and_device_ids() {
        let summary = SessionSummary {
            server_address: "https://onyx.example.com".to_string(),
            user_id: ObjectId([1; 16]),
            organization_id: ObjectId([2; 16]),
            username: "staff".to_string(),
            is_admin: false,
        };
        let info = SessionInfo::from_summary(&summary, ReplicaId([3; 16]), None);
        let json = serde_json::to_value(info).expect("SessionInfo must serialize");

        assert_eq!(json["username"], "staff");
        assert_eq!(json["userId"], ObjectId([1; 16]).to_string());
        assert_eq!(json["organizationId"], ObjectId([2; 16]).to_string());
        assert_eq!(
            json["deviceId"],
            uuid::Uuid::from_bytes([3; 16]).to_string()
        );
        assert!(json.get("accessToken").is_none());
        assert!(json.get("refreshToken").is_none());
        assert!(json["persistenceNotice"].is_null());
    }

    #[test]
    fn http_becomes_ws() {
        assert_eq!(
            relay_ws_endpoint_from_http("http://192.168.0.250:3000"),
            "ws://192.168.0.250:3000"
        );
    }

    #[test]
    fn https_becomes_wss() {
        assert_eq!(
            relay_ws_endpoint_from_http("https://onyx.example.com"),
            "wss://onyx.example.com"
        );
    }

    #[test]
    fn already_ws_style_is_left_unchanged() {
        // Defensive fallback path: an input that isn't http(s) at all
        // (e.g. someone hand-edited stored config to already be a ws://
        // address) must not be mangled into something malformed.
        assert_eq!(
            relay_ws_endpoint_from_http("ws://192.168.0.250:3000"),
            "ws://192.168.0.250:3000"
        );
    }
}
