//! Cloud Relay switchboard — the server side of Part II Chapter 8's
//! always-available fallback transport (§8.2, §8.3.1).
//!
//! # What this is
//! `sync-transport`'s `CloudRelayTransport` dials
//! `wss://<endpoint>/api/relay/<target-replica-uuid>` and then exchanges
//! `SyncMessage` frames (`crates/transports/sync-transport/src/cloud_relay.rs`).
//! Nothing served that path until now, which is why every Cloud Relay
//! connection failed and every client reported itself offline. This module
//! is the other half of that conversation.
//!
//! # Why the relay is allowed to read the frames it forwards
//! §8.5.1 resolves Q2 as transport-level encryption only: "The server (Cloud
//! Relay) can read payload contents in transit ... Chapter 7's CRDT merge
//! logic operates on plaintext payloads at the server as well as on-device."
//! So parsing each frame to route it is explicitly within the contract, not a
//! confidentiality violation. It is also necessary: routing needs
//! `target_replica`, and tenant isolation needs `organization_id`, both of
//! which live inside the frame.
//!
//! # Why this is a live switchboard, not a store-and-forward queue
//! A frame addressed to a replica that is not currently connected is dropped,
//! and that is the correct behaviour rather than a gap. Durability for
//! synchronization lives in the Outbox, not here: §7.7.1 resolves Q9 by
//! stating escalation "is never 'sent' and lost, it is durably queued until
//! delivered", and §6.2 makes the Outbox Relay retry-until-success. A relay
//! that also tried to persist undelivered sync frames would be a second,
//! weaker durability mechanism competing with the one the architecture
//! already designates — the same duplicated-enforcement mistake §8.7.1
//! rejects for quota. The relay moves bytes between replicas that are both
//! online; the Outbox is what makes eventual delivery certain.

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use sync_transport::SyncMessage;
use tokio::sync::{mpsc, RwLock};

use super::{validate_token, ApiError, ApiState};

/// One connected replica's inbound mailbox, plus the tenant it authenticated
/// as. The organization is held here so a forward can be refused without
/// having to re-read the target's token.
struct ConnectedReplica {
    organization_id: String,
    inbox: mpsc::UnboundedSender<Vec<u8>>,
}

/// Process-wide map of replica UUID -> live connection.
///
/// Deliberately in-memory and per-instance. A multi-instance deployment needs
/// replica presence in shared state (the natural home is the same Postgres
/// the rest of the API already uses) so two replicas landing on different
/// nodes can still reach each other; until that exists, a single api-server
/// instance is the supported topology for relay. Flagged here rather than
/// silently assumed, because the failure mode is subtle: everything works in
/// testing and in single-node deployment, and only breaks once the API is
/// scaled horizontally.
#[derive(Clone, Default)]
pub struct RelayRegistry {
    peers: Arc<RwLock<HashMap<uuid::Uuid, ConnectedReplica>>>,
}

impl RelayRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    async fn register(
        &self,
        id: uuid::Uuid,
        organization_id: String,
        inbox: mpsc::UnboundedSender<Vec<u8>>,
    ) {
        self.peers.write().await.insert(
            id,
            ConnectedReplica {
                organization_id,
                inbox,
            },
        );
    }

    async fn deregister(&self, id: &uuid::Uuid) {
        self.peers.write().await.remove(id);
    }

    /// Forwards one frame. Returns false when the target is absent or belongs
    /// to a different tenant — the caller logs and continues rather than
    /// tearing the connection down, since one undeliverable frame says
    /// nothing about the health of the connection that produced it.
    async fn forward(&self, target: &uuid::Uuid, sender_org: &str, frame: Vec<u8>) -> bool {
        let peers = self.peers.read().await;
        match peers.get(target) {
            // Tenant isolation (Part I §11.1, §8.6): a replica may only be
            // reached by a replica in the same organization. Checked against
            // the target's own authenticated organization, not against
            // anything the sender asserted in the frame.
            Some(peer) if peer.organization_id == sender_org => peer.inbox.send(frame).is_ok(),
            _ => false,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RelayAuth {
    pub token: String,
    /// The dialling replica's own id.
    ///
    /// Required, and deliberately not inferred from the first frame. Lazy
    /// registration looked simpler but is wrong twice over: a replica that
    /// only ever listens would never become addressable at all, and even
    /// between two replicas that both transmit there is a race where the
    /// first frame arrives before its target has finished registering and is
    /// silently dropped. Declaring identity in the handshake makes a
    /// connection addressable the moment it is open.
    #[serde(rename = "self")]
    pub self_replica: String,
}

/// `GET /api/relay/:target_id?token=...`
///
/// The path names the peer this connection wants to reach, matching the URL
/// `CloudRelayTransport::connect` builds. The caller's own identity is not in
/// the path; it is taken from the `sender_replica` field of the first frame
/// it sends (see `serve_relay`).
pub async fn relay_route(
    ws: WebSocketUpgrade,
    Path(target_id): Path<String>,
    Query(params): Query<RelayAuth>,
    State(state): State<ApiState>,
) -> Result<Response, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let claims = validate_token(&state, &params.token, "access").await?;

    let invalid = |what: &str| {
        ApiError::new(
            axum::http::StatusCode::BAD_REQUEST,
            "INVALID_REPLICA_ID",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation_id.clone(),
            serde_json::json!({ "message": format!("{what} must be a replica UUID") }),
        )
    };

    let default_target =
        uuid::Uuid::parse_str(&target_id).map_err(|_| invalid("relay path segment"))?;
    let self_replica =
        uuid::Uuid::parse_str(&params.self_replica).map_err(|_| invalid("self query parameter"))?;

    let organization_id = claims.organization_id;
    Ok(ws.on_upgrade(move |socket| {
        serve_relay(socket, state, organization_id, self_replica, default_target)
    }))
}

async fn serve_relay(
    socket: WebSocket,
    state: ApiState,
    authenticated_org: String,
    self_id: uuid::Uuid,
    default_target: uuid::Uuid,
) {
    let (mut sink, mut stream) = socket.split();
    let (inbox_tx, mut inbox_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    // Addressable immediately, before any frame is read — see `RelayAuth`.
    state
        .relay_registry
        .register(self_id, authenticated_org.clone(), inbox_tx)
        .await;

    loop {
        tokio::select! {
            incoming = stream.next() => {
                let frame = match incoming {
                    Some(Ok(Message::Binary(bytes))) => bytes,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    // SyncMessage is a binary wire format (§3.2's byte-level
                    // layout). Text frames are not part of the protocol and
                    // are ignored rather than guessed at.
                    _ => continue,
                };

                let message = match SyncMessage::deserialize(&frame) {
                    Ok(message) => message,
                    Err(_) => {
                        tracing::warn!(
                            organization_id = %authenticated_org,
                            "relay: dropping frame that is not a valid SyncMessage"
                        );
                        continue;
                    }
                };

                let frame_org = uuid::Uuid::from_bytes(message.organization_id.0).to_string();
                // A frame claiming a different tenant than the token it
                // arrived under is not a routing mistake to be logged and
                // skipped — it is an attempt to cross a tenant boundary, so
                // the connection ends.
                if frame_org != authenticated_org {
                    tracing::warn!(
                        authenticated_org = %authenticated_org,
                        frame_org = %frame_org,
                        "relay: closing connection after cross-tenant frame"
                    );
                    break;
                }

                // A connection may only ever speak as the replica it declared
                // in the handshake. Without this one authenticated client
                // could forge frames from every replica in its organization
                // simply by varying `sender_replica`.
                let sender = uuid::Uuid::from_bytes(message.sender_replica.0);
                if sender != self_id {
                    tracing::warn!(
                        declared = %self_id,
                        claimed = %sender,
                        "relay: closing connection after sender_replica mismatch"
                    );
                    break;
                }

                // `target_replica` is optional in the wire format; the path
                // segment is the fallback, which is what a client that dialled
                // a specific peer already told us.
                let target = message
                    .target_replica
                    .map(|id| uuid::Uuid::from_bytes(id.0))
                    .unwrap_or(default_target);

                if !state.relay_registry.forward(&target, &authenticated_org, frame).await {
                    // Expected whenever the peer is simply not online. The
                    // sender's Outbox is what retries; see the module note.
                    tracing::debug!(
                        target = %target,
                        "relay: target not connected, frame dropped"
                    );
                }
            }
            outbound = inbox_rx.recv() => {
                match outbound {
                    Some(frame) => {
                        if sink.send(Message::Binary(frame)).await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }

    state.relay_registry.deregister(&self_id).await;
}
