//! End-to-end proof that the Cloud Relay switchboard actually moves a frame
//! between two replicas, and refuses the cases it must refuse.
//!
//! Compiling is not evidence that a relay relays. These tests bind a real
//! server, open real WebSocket connections with `tokio-tungstenite` (the same
//! client library the desktop shell's `RelaySocketFactory` uses), and assert
//! on bytes that actually crossed the process boundary.

use std::{net::SocketAddr, time::Duration};

use futures_util::{SinkExt, StreamExt};
use platform_kernel::{ObjectId, ReplicaId, SchemaVersion, Timestamp};
use sync_transport::{message::MessageId, SyncMessage, SyncMessageType};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const BOOTSTRAP_TOKEN: &str = "relay-test-bootstrap";

/// Boots an api-server on an ephemeral port against a throwaway SQLite file,
/// bootstraps an admin, and returns the address plus a valid access token.
async fn start_server(db_label: &str) -> (SocketAddr, String) {
    std::env::set_var("ONYX_BOOTSTRAP_TOKEN", BOOTSTRAP_TOKEN);

    // A file rather than `:memory:` — the pool opens multiple connections and
    // each would otherwise get its own private empty database.
    let db_path = std::env::temp_dir().join(format!("onyx-relay-test-{db_label}.db"));
    let _ = std::fs::remove_file(&db_path);
    let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let state = api_server::routes::ApiState::new(&database_url)
        .await
        .expect("api state");
    let app = api_server::routes::router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let http = reqwest::Client::new();
    let base = format!("http://{addr}");

    http.post(format!("{base}/api/admin/bootstrap"))
        .header("x-onyx-bootstrap-token", BOOTSTRAP_TOKEN)
        .json(&serde_json::json!({"username": "relay-tester", "password": "relay-test-password"}))
        .send()
        .await
        .expect("bootstrap request");

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "relay-tester", "password": "relay-test-password"}))
        .send()
        .await
        .expect("login request")
        .json()
        .await
        .expect("login body");

    let token = login["access_token"]
        .as_str()
        .expect("access_token in login response")
        .to_string();

    (addr, token)
}

fn frame(sender: ReplicaId, target: Option<ReplicaId>, org: ObjectId, payload: &[u8]) -> Vec<u8> {
    SyncMessage {
        message_id: MessageId([7u8; 16]),
        version: SchemaVersion("1.0".to_string()),
        message_type: SyncMessageType::OperationBatch,
        organization_id: org,
        sender_replica: sender,
        target_replica: target,
        payload: payload.to_vec(),
        timestamp: Timestamp(0),
        signature: None,
    }
    .serialize()
}

/// The organization the bootstrap admin is created in. Relay frames must
/// carry this or they are treated as cross-tenant.
fn test_org() -> ObjectId {
    let uuid = uuid::Uuid::parse_str(api_server::routes::ORGANIZATION_ID).unwrap();
    ObjectId(*uuid.as_bytes())
}

async fn open(
    addr: SocketAddr,
    me: ReplicaId,
    target: ReplicaId,
    token: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let target_uuid = uuid::Uuid::from_bytes(target.0);
    let self_uuid = uuid::Uuid::from_bytes(me.0);
    let url = format!("ws://{addr}/api/relay/{target_uuid}?token={token}&self={self_uuid}");
    let (stream, _) = connect_async(url).await.expect("relay handshake");
    stream
}

#[tokio::test]
async fn relay_forwards_a_frame_between_two_replicas() {
    let (addr, token) = start_server("forward").await;

    let alice = ReplicaId([1u8; 16]);
    let bob = ReplicaId([2u8; 16]);
    let org = test_org();

    let mut alice_ws = open(addr, alice, bob, &token).await;
    let mut bob_ws = open(addr, bob, alice, &token).await;

    // Both replicas are addressable the moment their sockets open, so no
    // warm-up frame and no particular ordering is required. This assertion is
    // the regression guard for the lazy-registration design that preceded it,
    // under which this exact exchange silently dropped Bob's frame because
    // Alice had not yet transmitted anything.
    bob_ws
        .send(Message::Binary(frame(bob, Some(alice), org, b"hello-from-bob")))
        .await
        .unwrap();

    let to_alice = tokio::time::timeout(Duration::from_secs(5), alice_ws.next())
        .await
        .expect("alice should receive within timeout")
        .expect("stream open")
        .expect("frame ok");
    let decoded = SyncMessage::deserialize(&to_alice.into_data()).expect("valid SyncMessage");
    assert_eq!(decoded.payload, b"hello-from-bob");
    assert_eq!(decoded.sender_replica, bob);

    // Now the reverse direction, which is the case the desktop client
    // actually performs first.
    alice_ws
        .send(Message::Binary(frame(alice, Some(bob), org, b"hello-from-alice")))
        .await
        .unwrap();

    let to_bob = tokio::time::timeout(Duration::from_secs(5), bob_ws.next())
        .await
        .expect("bob should receive within timeout")
        .expect("stream open")
        .expect("frame ok");
    let decoded = SyncMessage::deserialize(&to_bob.into_data()).expect("valid SyncMessage");
    assert_eq!(decoded.payload, b"hello-from-alice");
    assert_eq!(decoded.sender_replica, alice);
}

#[tokio::test]
async fn relay_drops_frames_for_an_absent_peer_without_killing_the_sender() {
    let (addr, token) = start_server("absent").await;

    let alice = ReplicaId([3u8; 16]);
    let ghost = ReplicaId([9u8; 16]);
    let org = test_org();

    let mut alice_ws = open(addr, alice, ghost, &token).await;

    // Nobody is connected as `ghost`. The frame is undeliverable, but that
    // says nothing about Alice's connection — the Outbox is what retries
    // (Part II §7.7.1), so the relay must not tear her down.
    alice_ws
        .send(Message::Binary(frame(alice, Some(ghost), org, b"into-the-void")))
        .await
        .unwrap();

    // Still usable afterwards: a second send must succeed.
    alice_ws
        .send(Message::Binary(frame(alice, Some(ghost), org, b"still-alive")))
        .await
        .expect("connection should survive an undeliverable frame");

    // And nothing should have come back.
    let nothing = tokio::time::timeout(Duration::from_millis(300), alice_ws.next()).await;
    assert!(nothing.is_err(), "no frame should be delivered for an absent peer");
}

#[tokio::test]
async fn relay_closes_connection_on_cross_tenant_frame() {
    let (addr, token) = start_server("tenant").await;

    let alice = ReplicaId([4u8; 16]);
    let bob = ReplicaId([5u8; 16]);
    let foreign_org = ObjectId([0xAB; 16]);

    let mut alice_ws = open(addr, alice, bob, &token).await;

    // Authenticated as the test org, but the frame claims another tenant.
    // This is a boundary-crossing attempt, not a routing mistake, so the
    // connection must end rather than the frame merely being skipped.
    alice_ws
        .send(Message::Binary(frame(alice, Some(bob), foreign_org, b"wrong-tenant")))
        .await
        .unwrap();

    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match alice_ws.next().await {
                Some(Ok(Message::Close(_))) | None => return true,
                Some(Err(_)) => return true,
                Some(Ok(_)) => continue,
            }
        }
    })
    .await
    .expect("server should close the connection");

    assert!(closed, "cross-tenant frame must terminate the connection");
}
