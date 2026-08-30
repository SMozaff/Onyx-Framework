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

/// Boots an api-server on an ephemeral port against a throwaway SQLite file
/// and authenticates its intentionally seeded test-drive administrator.
async fn start_server(db_label: &str) -> (SocketAddr, String) {
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

    let login: serde_json::Value = http
        .post(format!("{base}/api/auth/login"))
        .json(&serde_json::json!({"username": "All-Father", "password": "passvord0000"}))
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

/// The organization the seeded test-drive admin is created in. Relay frames
/// must carry this or they are treated as cross-tenant.
fn test_org() -> ObjectId {
    let uuid = uuid::Uuid::parse_str(api_server::routes::ORGANIZATION_ID).unwrap();
    ObjectId(*uuid.as_bytes())
}

/// Mints a real relay ticket over `/api/relay-ticket` (H4(b)) -- the same
/// call the desktop client's `TungsteniteRelaySocketFactory` makes before
/// ever dialling the WebSocket -- rather than reusing the raw access token,
/// so these tests exercise the actual production auth path.
async fn mint_ticket(addr: SocketAddr, token: &str, target: ReplicaId) -> String {
    let target_uuid = uuid::Uuid::from_bytes(target.0);
    let response: serde_json::Value = reqwest::Client::new()
        .post(format!("http://{addr}/api/relay-ticket"))
        .bearer_auth(token)
        .json(&serde_json::json!({ "target_id": target_uuid.to_string() }))
        .send()
        .await
        .expect("ticket request")
        .json()
        .await
        .expect("ticket body");
    response["ticket"]
        .as_str()
        .expect("ticket in response")
        .to_string()
}

async fn open(
    addr: SocketAddr,
    me: ReplicaId,
    target: ReplicaId,
    token: &str,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let ticket = mint_ticket(addr, token, target).await;
    let target_uuid = uuid::Uuid::from_bytes(target.0);
    let self_uuid = uuid::Uuid::from_bytes(me.0);
    let url = format!("ws://{addr}/api/relay/{target_uuid}?ticket={ticket}&self={self_uuid}");
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
        .send(Message::Binary(frame(
            bob,
            Some(alice),
            org,
            b"hello-from-bob",
        )))
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
        .send(Message::Binary(frame(
            alice,
            Some(bob),
            org,
            b"hello-from-alice",
        )))
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
        .send(Message::Binary(frame(
            alice,
            Some(ghost),
            org,
            b"into-the-void",
        )))
        .await
        .unwrap();

    // Still usable afterwards: a second send must succeed.
    alice_ws
        .send(Message::Binary(frame(
            alice,
            Some(ghost),
            org,
            b"still-alive",
        )))
        .await
        .expect("connection should survive an undeliverable frame");

    // And nothing should have come back.
    let nothing = tokio::time::timeout(Duration::from_millis(300), alice_ws.next()).await;
    assert!(
        nothing.is_err(),
        "no frame should be delivered for an absent peer"
    );
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
        .send(Message::Binary(frame(
            alice,
            Some(bob),
            foreign_org,
            b"wrong-tenant",
        )))
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

/// H4(b): a relay ticket authorizes exactly one WebSocket connection
/// attempt. A second connection attempt with the exact same ticket -- the
/// literal replay scenario a leaked URL enables -- must be rejected, even
/// though the ticket has not yet expired and is otherwise well-formed.
#[tokio::test]
async fn relay_ticket_cannot_be_redeemed_twice() {
    let (addr, token) = start_server("ticket-reuse").await;
    let alice = ReplicaId([11u8; 16]);
    let bob = ReplicaId([12u8; 16]);
    let ticket = mint_ticket(addr, &token, bob).await;
    let self_uuid = uuid::Uuid::from_bytes(alice.0);
    let target_uuid = uuid::Uuid::from_bytes(bob.0);
    let url = format!("ws://{addr}/api/relay/{target_uuid}?ticket={ticket}&self={self_uuid}");

    // First use: succeeds, exactly as `open()` proves elsewhere in this file.
    let (_first, _) = connect_async(&url)
        .await
        .expect("first redemption of a fresh ticket must succeed");

    // Second use of the identical ticket: must be refused outright, not
    // merely produce a connection that is silently unusable -- the HTTP
    // upgrade itself must fail.
    let second = connect_async(&url).await;
    assert!(
        second.is_err(),
        "a relay ticket must not be redeemable a second time"
    );
}

/// H4(b): a ticket is scoped to the specific target it was minted for. A
/// still-fresh, never-redeemed ticket presented against a *different*
/// target path must be rejected -- otherwise scope is cosmetic and any
/// valid ticket could be retargeted by an attacker who intercepted it.
#[tokio::test]
async fn relay_ticket_cannot_be_used_against_a_different_target() {
    let (addr, token) = start_server("ticket-scope").await;
    let alice = ReplicaId([13u8; 16]);
    let intended_target = ReplicaId([14u8; 16]);
    let other_target = ReplicaId([15u8; 16]);

    let ticket = mint_ticket(addr, &token, intended_target).await;
    let self_uuid = uuid::Uuid::from_bytes(alice.0);
    let other_target_uuid = uuid::Uuid::from_bytes(other_target.0);
    let mismatched_url =
        format!("ws://{addr}/api/relay/{other_target_uuid}?ticket={ticket}&self={self_uuid}");

    let attempt = connect_async(&mismatched_url).await;
    assert!(
        attempt.is_err(),
        "a ticket minted for one target must be rejected against a different target path"
    );
}
