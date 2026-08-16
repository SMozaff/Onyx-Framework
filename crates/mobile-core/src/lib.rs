//! `mobile-core` — the C-ABI FFI library for iOS/Android, per Team
//! Prompt 5 §3.3/§4.2. A thin wrapper around `client-composition`'s
//! `AppState`, mirroring `desktop-shell`'s composition but exposed via
//! `extern "C"` functions instead of Tauri commands (ruling C1,
//! `DECISIONS.md`).
//!
//! # Verification status (flagged, not silently assumed)
//! This crate compiles and passes strict clippy in this sandbox (which
//! has no Xcode/Android NDK), matching Team 4's own established
//! precedent for mobile FFI: real, correct Rust code, typecheck-verified,
//! but not linked/run against an actual iOS or Android target. The
//! *logic* (dispatch through `AppState`'s real registries) is exercised
//! by real Rust integration tests (`tests/ffi_integration.rs`), calling
//! the same `extern "C"` functions directly from safe Rust (not through
//! an actual C/Swift/Kotlin caller).
//!
//! # Module layout
//! Shared types (`MobileApp`, `EventSubscription`, `MobileConfig`) and
//! helpers (`cstr_to_string`/`string_to_cstr`) live in this file;
//! `mobile_core_new`/`mobile_core_free`/`mobile_core_free_string` too,
//! since they operate directly on those shared types. Each FFI entry
//! point beyond those is split into its own file by concern:
//! `ffi_commands.rs`, `ffi_queries.rs`, `ffi_events.rs`,
//! `ios_background.rs`, `android_workmanager.rs` — plus the four P2P
//! transport stub modules (`ios_multipeer.rs`, `android_wifi_direct.rs`,
//! `ios_ble.rs`, `android_ble.rs`). This was originally one flat
//! `lib.rs`; split for the requested file structure with no behavior
//! change (verified: the full `tests/ffi_integration.rs` suite re-passes
//! unmodified after the split — see `DECISIONS.md`).

use std::ffi::{c_char, CStr, CString};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::Arc;

use client_composition::app_state::{AppState, AppStateConfig};
use platform_kernel::{OrganizationId, ReplicaId};
use sqlx::sqlite::SqlitePoolOptions;
use tokio::runtime::Runtime;

pub mod android_ble;
pub mod android_wifi_direct;
pub mod android_workmanager;
pub mod ffi_commands;
pub mod ffi_events;
pub mod ffi_mobile;
pub mod ffi_queries;
pub mod ffi_secure_storage;
pub mod ios_background;
pub mod ios_ble;
pub mod ios_multipeer;

pub use android_workmanager::mobile_core_android_do_work;
pub use ffi_commands::mobile_core_execute_command;
pub use ffi_events::{
    mobile_core_subscribe_events, mobile_core_trigger_sync, mobile_core_unsubscribe,
};
pub use ffi_mobile::{
    mobile_core_get_sync_status, mobile_core_list_aggregates, mobile_core_list_conflicts,
    mobile_core_resolve_conflict,
};
pub use ffi_queries::mobile_core_execute_query;
pub use ios_background::mobile_core_ios_background_sync;

/// Opaque handle returned to the caller, matching the C header's
/// `typedef struct MobileApp MobileApp;` (Team Prompt 5 §3.3). Holds its
/// own dedicated tokio runtime (mobile apps have no ambient async
/// runtime the way a `#[tokio::main]` binary does) plus the same
/// `AppState` `desktop-shell` builds.
pub struct MobileApp {
    pub(crate) runtime: Runtime,
    pub(crate) state: Arc<AppState>,
    pub(crate) background_task: tokio::task::JoinHandle<()>,
}

/// Opaque handle for an active event subscription, matching the header's
/// `typedef struct EventSubscription EventSubscription;`. Holds the
/// `JoinHandle` for the forwarding task so `mobile_core_unsubscribe` can
/// abort it — `client-composition`'s `Subscription` itself has no
/// `unsubscribe()` (it simply stops being polled when dropped), so the
/// task doing the polling is what needs to be told to stop.
pub struct EventSubscription {
    pub(crate) task: tokio::task::JoinHandle<()>,
}

/// The Flutter application owns one mobile core instance per process. Native
/// background schedulers cannot carry a Dart pointer after suspension, so the
/// active instance is registered here for the platform wrappers. The app must
/// call `mobile_core_free` only after background callbacks have been stopped.
static REGISTERED_BACKGROUND_APP: AtomicPtr<MobileApp> = AtomicPtr::new(std::ptr::null_mut());

/// Configuration passed as `config_json` to `mobile_core_new`, per Team
/// Prompt 5 §3.3's doc comment ("JSON string with configuration
/// (endpoint, sync interval, etc.)"). Not specified further by the
/// prompt; shaped around what `AppStateConfig` actually needs.
#[derive(serde::Deserialize)]
struct MobileConfig {
    organization_id: OrganizationId,
    cloud_relay_endpoint: String,
    #[serde(default = "default_sync_interval_secs")]
    sync_interval_secs: u64,
}

fn default_sync_interval_secs() -> u64 {
    60
}

/// Converts a C string pointer to an owned `String`, or returns `None`
/// if the pointer is null or not valid UTF-8. Every FFI entry point
/// checks for null and invalid UTF-8 explicitly — an `extern "C"`
/// function that dereferences a null/dangling/non-UTF-8 pointer without
/// checking is undefined behavior, not just a Rust-level error.
///
/// # Safety
/// `ptr` must be a valid, NUL-terminated C string pointer, or null.
pub(crate) unsafe fn cstr_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(|s| s.to_string())
}

/// Converts a Rust `String` into a raw pointer the caller owns and must
/// free via `mobile_core_free_string`.
pub(crate) fn string_to_cstr(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(), // Embedded NUL byte; caller sees a null pointer rather than UB.
    }
}

/// Initializes the mobile core. Returns a handle, or null on failure
/// (invalid arguments, or the async setup work — opening the SQLite pool,
/// applying migrations — failing). Team Prompt 5 §3.3.
///
/// # Safety
/// `db_path` and `config_json` must each be a valid, NUL-terminated C
/// string pointer.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_new(
    db_path: *const c_char,
    config_json: *const c_char,
) -> *mut MobileApp {
    let Some(db_path) = cstr_to_string(db_path) else {
        return std::ptr::null_mut();
    };
    let Some(config_json) = cstr_to_string(config_json) else {
        return std::ptr::null_mut();
    };
    let Ok(config) = serde_json::from_str::<MobileConfig>(&config_json) else {
        return std::ptr::null_mut();
    };

    let runtime = match Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return std::ptr::null_mut(),
    };

    let state = runtime.block_on(async {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&format!("sqlite://{db_path}?mode=rwc"))
            .await
            .ok()?;

        let schema =
            include_str!("../../../migrations/sqlite/20260101000000_initial_schema.up.sql");
        for stmt in schema.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            // Second launch: tables already exist; only a first-launch
            // failure to even connect (handled above) is treated as
            // fatal, matching desktop-shell's identical rationale.
            let _ = sqlx::query(stmt).execute(&pool).await;
        }

        let local_replica = ReplicaId::new_random();

        // Phones are the case LAN discovery exists for: a handset joining the
        // site Wi-Fi has no way to know the other replicas' addresses, and no
        // one is going to type them in. Binding can fail on a hardened or
        // restricted device, which costs automatic peer-finding, not sync.
        let lan_discovery: Option<Arc<dyn sync_transport::Discovery>> =
            match lan_discovery::LanDiscovery::start(
                local_replica,
                config.organization_id,
                3000,
                None,
            )
            .await
            {
                Ok(d) => Some(Arc::new(d)),
                Err(e) => {
                    tracing::warn!(error = %e, "LAN discovery unavailable; relay only");
                    None
                }
            };

        let app_config = AppStateConfig {
            local_replica,
            organization_id: config.organization_id,
            local_discovery: lan_discovery,
            sync_agent_config: client_composition::sync_agent::SyncAgentConfig {
                sync_interval: std::time::Duration::from_secs(config.sync_interval_secs),
                ..Default::default()
            },
            event_bus_capacity: 1024,
            cloud_relay_endpoint: config.cloud_relay_endpoint,
            // Same flagged gap as desktop-shell (DECISIONS.md): no
            // production AuthorityProvider/RelaySocketFactory exists yet
            // for either client. mobile-core additionally has no
            // real ffi_secure_storage-backed AuthorityProvider wired in
            // here either — see ffi_secure_storage.rs's own doc comment.
            cloud_relay_auth_provider: Arc::new(
                sync_transport::placeholder_types::StaticAuthorityProvider(String::new()),
            ),
            cloud_relay_socket_factory: Arc::new(NotYetImplementedSocketFactory),
        };

        Some(Arc::new(AppState::new(pool, app_config)))
    });

    let Some(state) = state else {
        return std::ptr::null_mut();
    };

    let sync_agent = Arc::clone(&state.sync_agent);
    let background_task = runtime.spawn(async move { sync_agent.run().await });
    let app = Box::into_raw(Box::new(MobileApp {
        runtime,
        state,
        background_task,
    }));
    REGISTERED_BACKGROUND_APP.store(app, Ordering::Release);
    app
}

/// Runs one background synchronization cycle against the registered mobile
/// core instance. Intended for iOS `BGAppRefreshTask` and Android WorkManager.
#[no_mangle]
pub extern "C" fn mobile_core_background_sync_registered(task_id: u64) -> i32 {
    let handle = REGISTERED_BACKGROUND_APP.load(Ordering::Acquire);
    if handle.is_null() {
        return 0;
    }
    // SAFETY: the Flutter app registers exactly one process-lifetime handle
    // and clears it before freeing the allocation.
    unsafe { ios_background::mobile_core_ios_background_sync(handle, task_id) }
}

/// Frees the mobile core handle. Team Prompt 5 §3.3.
///
/// # Safety
/// `handle` must be a pointer previously returned by `mobile_core_new`,
/// not yet freed, and not used again after this call.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_free(handle: *mut MobileApp) {
    if !handle.is_null() {
        let _ = REGISTERED_BACKGROUND_APP.compare_exchange(
            handle,
            std::ptr::null_mut(),
            Ordering::AcqRel,
            Ordering::Acquire,
        );
        let app = Box::from_raw(handle);
        app.background_task.abort();
        drop(app);
    }
}

/// Frees a string returned by the mobile core. Team Prompt 5 §3.3.
///
/// # Safety
/// `str` must be a pointer previously returned by one of this crate's
/// functions (`mobile_core_execute_command`/`_execute_query`), not yet
/// freed, and not used again after this call.
#[no_mangle]
pub unsafe extern "C" fn mobile_core_free_string(str: *mut c_char) {
    if !str.is_null() {
        drop(CString::from_raw(str));
    }
}

/// Placeholder `RelaySocketFactory` — see `desktop-shell`'s identical
/// type and `DECISIONS.md`'s `AppState` entry for the flagged gap this
/// represents (no production Cloud Relay socket/auth implementation
/// exists yet for either client).
struct NotYetImplementedSocketFactory;

#[async_trait::async_trait]
impl sync_transport::cloud_relay::RelaySocketFactory for NotYetImplementedSocketFactory {
    async fn connect(
        &self,
        _relay_url: &str,
        _peer: &sync_transport::PeerInfo,
        _bearer_token: &str,
        _timeout: std::time::Duration,
    ) -> Result<Box<dyn sync_transport::cloud_relay::RelaySocket>, sync_transport::TransportError>
    {
        Err(sync_transport::TransportError::Unreachable)
    }
}
