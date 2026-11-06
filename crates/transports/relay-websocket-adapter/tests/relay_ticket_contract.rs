//! Relay ticket contract tests for `TungsteniteRelaySocketFactory`.
//!
//! The mock in this file stands in for the Worker's `POST /api/relay-ticket`
//! and `GET /api/relay/:target` routes. Each test checks what the mock
//! received and what the client returned. Tests that need the real Worker are
//! at the bottom and marked `pending-manual`.
//!
//! Invariants checked on every path:
//! - the bearer access token is sent only in the HTTPS `Authorization` header;
//! - the WebSocket URL carries the single-use ticket and the self replica,
//!   never the access token;
//! - no error message contains the token.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use axum::{
    extract::{ws::WebSocketUpgrade, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use platform_kernel::{OrganizationId, ReplicaId, Timestamp};
use relay_websocket_adapter::TungsteniteRelaySocketFactory;
use serde_json::{json, Value};
use sync_transport::{cloud_relay::RelaySocketFactory, PeerInfo, TransportError};
use tokio::net::TcpListener;

const ACCESS_TOKEN: &str = "generated-access-token-for-contract-tests";
const TICKET: &str = "generated-single-use-ticket";

#[derive(Clone, Copy)]
enum TicketMode {
    Ok,
    Unauthorized,
    Forbidden,
    NotMigrated,
    Slow(Duration),
}

struct MockWorker {
    mode: TicketMode,
    ticket_calls: AtomicUsize,
    ws_attempts: AtomicUsize,
    last_authorization: Mutex<Option<String>>,
    last_ticket_body: Mutex<Option<String>>,
    last_ws_query: Mutex<Option<HashMap<String, String>>>,
}

async fn ticket_route(
    State(worker): State<Arc<MockWorker>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    worker.ticket_calls.fetch_add(1, Ordering::SeqCst);
    *worker.last_authorization.lock().unwrap() = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    *worker.last_ticket_body.lock().unwrap() = Some(body.to_string());
    match worker.mode {
        TicketMode::Ok => (
            StatusCode::OK,
            Json(json!({ "ticket": TICKET, "expires_in": 30 })),
        ),
        TicketMode::Unauthorized => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": "UNAUTHORIZED" })),
        ),
        TicketMode::Forbidden => (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "CLIENT_CAPABILITY_REQUIRED" })),
        ),
        TicketMode::NotMigrated => (
            StatusCode::NOT_IMPLEMENTED,
            Json(json!({ "error": "ROUTE_NOT_MIGRATED", "route": "/api/relay-ticket" })),
        ),
        TicketMode::Slow(delay) => {
            tokio::time::sleep(delay).await;
            (
                StatusCode::OK,
                Json(json!({ "ticket": TICKET, "expires_in": 30 })),
            )
        }
    }
}

async fn relay_route(
    State(worker): State<Arc<MockWorker>>,
    Query(query): Query<HashMap<String, String>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    worker.ws_attempts.fetch_add(1, Ordering::SeqCst);
    *worker.last_ws_query.lock().unwrap() = Some(query);
    ws.on_upgrade(|_socket| async {})
}

async fn start(mode: TicketMode) -> (Arc<MockWorker>, SocketAddr) {
    let worker = Arc::new(MockWorker {
        mode,
        ticket_calls: AtomicUsize::new(0),
        ws_attempts: AtomicUsize::new(0),
        last_authorization: Mutex::new(None),
        last_ticket_body: Mutex::new(None),
        last_ws_query: Mutex::new(None),
    });
    let app = Router::new()
        .route("/api/relay-ticket", post(ticket_route))
        .route("/api/relay/:target", get(relay_route))
        .with_state(Arc::clone(&worker));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (worker, address)
}

fn peer() -> PeerInfo {
    PeerInfo {
        id: ReplicaId::new_random(),
        organization_id: OrganizationId::new_random(),
        endpoints: vec!["127.0.0.1:4433".parse().unwrap()],
        service_name: None,
        mac_address: None,
        bluetooth_address: None,
        last_seen: Timestamp::now(),
        from_cloud: false,
    }
}

async fn connect(
    address: SocketAddr,
    timeout: Duration,
) -> (ReplicaId, PeerInfo, Result<(), TransportError>) {
    let local = ReplicaId::new_random();
    let factory = TungsteniteRelaySocketFactory::new(local);
    let target = peer();
    let url = format!(
        "ws://{address}/api/relay/{}",
        uuid::Uuid::from_bytes(target.id.0)
    );
    let result = factory
        .connect(&url, &target, ACCESS_TOKEN, timeout)
        .await
        .map(|_socket| ());
    (local, target, result)
}

#[tokio::test]
async fn ticket_then_handshake_sends_token_only_in_header_and_ticket_in_url() {
    let (worker, address) = start(TicketMode::Ok).await;
    let (local, target, result) = connect(address, Duration::from_secs(5)).await;

    assert!(result.is_ok(), "handshake with a valid ticket must succeed");
    assert_eq!(worker.ticket_calls.load(Ordering::SeqCst), 1);
    assert_eq!(worker.ws_attempts.load(Ordering::SeqCst), 1);

    let authorization = worker.last_authorization.lock().unwrap().clone();
    assert_eq!(
        authorization.as_deref(),
        Some(format!("Bearer {ACCESS_TOKEN}").as_str())
    );

    let body = worker.last_ticket_body.lock().unwrap().clone().unwrap();
    assert!(!body.contains(ACCESS_TOKEN));
    assert!(body.contains(&uuid::Uuid::from_bytes(target.id.0).to_string()));
    assert!(body.contains(&uuid::Uuid::from_bytes(local.0).to_string()));

    let query = worker.last_ws_query.lock().unwrap().clone().unwrap();
    assert_eq!(query.get("ticket").map(String::as_str), Some(TICKET));
    assert_eq!(
        query.get("self").map(String::as_str),
        Some(uuid::Uuid::from_bytes(local.0).to_string().as_str())
    );
    assert!(
        !query.values().any(|v| v.contains(ACCESS_TOKEN)),
        "access token must never appear in the WebSocket URL"
    );
}

#[tokio::test]
async fn ticket_401_fails_before_any_websocket_attempt_without_leaking_token() {
    let (worker, address) = start(TicketMode::Unauthorized).await;
    let (_, _, result) = connect(address, Duration::from_secs(5)).await;

    let error = result.expect_err("401 must fail");
    let message = error.to_string();
    assert!(message.contains("401"), "error names the status: {message}");
    assert!(!message.contains(ACCESS_TOKEN));
    assert_eq!(worker.ws_attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn ticket_403_fails_before_any_websocket_attempt_without_leaking_token() {
    // 403 is the default-deny result for a user class without a mobile
    // capability grant (see deploy/cloudflare/edge-gateway/src/mobile-capability.ts).
    let (worker, address) = start(TicketMode::Forbidden).await;
    let (_, _, result) = connect(address, Duration::from_secs(5)).await;

    let error = result.expect_err("403 must fail");
    let message = error.to_string();
    assert!(message.contains("403"), "error names the status: {message}");
    assert!(!message.contains(ACCESS_TOKEN));
    assert_eq!(worker.ws_attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn ticket_501_reports_route_not_migrated_without_leaking_token() {
    let (worker, address) = start(TicketMode::NotMigrated).await;
    let (_, _, result) = connect(address, Duration::from_secs(5)).await;

    let message = result.expect_err("501 must fail").to_string();
    assert!(message.contains("ROUTE_NOT_MIGRATED"), "{message}");
    assert!(!message.contains(ACCESS_TOKEN));
    assert_eq!(worker.ws_attempts.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn slow_ticket_endpoint_times_out_before_any_websocket_attempt() {
    let (worker, address) = start(TicketMode::Slow(Duration::from_secs(3))).await;
    let (_, _, result) = connect(address, Duration::from_millis(200)).await;

    assert!(
        matches!(result, Err(TransportError::Timeout)),
        "a slow ticket mint must surface as Timeout"
    );
    assert_eq!(worker.ws_attempts.load(Ordering::SeqCst), 0);
}

// ---- pending-manual: require the deployed Worker ------------------------
//
// These cases need a live Worker origin and a test account whose class has a
// mobile capability grant. They cannot run in this build environment and
// must not be reported as passing from a local mock. Run them manually with:
//   ONYX_LIVE_RELAY_URL=wss://<origin>/api/relay/<target> \
//   ONYX_LIVE_ACCESS_TOKEN=<test token, not stored in the repo> \
//   cargo test -p relay-websocket-adapter --test relay_ticket_contract -- --ignored

#[tokio::test]
#[ignore = "pending-manual: needs the deployed Worker and a granted test account (not available here)"]
async fn live_worker_ticket_and_handshake_pending_manual() {
    let relay_url = std::env::var("ONYX_LIVE_RELAY_URL").expect("set ONYX_LIVE_RELAY_URL");
    let token = std::env::var("ONYX_LIVE_ACCESS_TOKEN").expect("set ONYX_LIVE_ACCESS_TOKEN");
    let factory = TungsteniteRelaySocketFactory::new(ReplicaId::new_random());
    let result = factory
        .connect(&relay_url, &peer(), &token, Duration::from_secs(15))
        .await;
    assert!(result.is_ok(), "live handshake failed (token not shown)");
}

#[tokio::test]
#[ignore = "pending-manual: needs the deployed Worker and an account without a grant (not available here)"]
async fn live_worker_ungranted_class_gets_403_pending_manual() {
    let relay_api = std::env::var("ONYX_LIVE_RELAY_URL").expect("set ONYX_LIVE_RELAY_URL");
    let token = std::env::var("ONYX_LIVE_ACCESS_TOKEN").expect("set ONYX_LIVE_ACCESS_TOKEN");
    let factory = TungsteniteRelaySocketFactory::new(ReplicaId::new_random());
    let result = factory
        .connect(&relay_api, &peer(), &token, Duration::from_secs(15))
        .await;
    let message = result.err().map(|e| e.to_string()).unwrap_or_default();
    assert!(
        message.contains("403"),
        "expected default-deny 403: {message}"
    );
}
