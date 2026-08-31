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
    http::StatusCode,
    response::Response,
    Json,
};
use futures_util::{SinkExt, StreamExt};
use security_adapter::Ed25519JwtCodec;
use serde::{Deserialize, Serialize};
use sync_transport::SyncMessage;
use tokio::sync::{mpsc, RwLock};

use super::{
    authenticate_headers, unix_seconds, validate_token, ApiError, ApiState, ProjectionPool,
    TokenClaims, TokenScope,
};

/// One connected replica's inbound mailbox, plus the tenant it authenticated
/// as. The organization is held here so a forward can be refused without
/// having to re-read the target's token.
struct ConnectedReplica {
    organization_id: String,
    inbox: mpsc::UnboundedSender<Vec<u8>>,
}

/// Process-wide map of replica UUID -> live connection.
///
/// Deliberately in-memory and per-instance -- and, as of hardening track H3,
/// this is now the actual, enforced production topology, not an aspirational
/// note about a known limitation. `/api/relay/:target_id` is served by
/// `deploy/helm/onyx-api-relay`, a dedicated Deployment hardcoded to exactly
/// one replica (see that chart's `templates/deployment.yaml`), decoupled
/// from `onyx-api`'s own horizontally-autoscaled Rollout. A second Ingress
/// object routes the `/api/relay` path specifically to it, so every relay
/// WebSocket connection lands on the same process regardless of how many
/// ordinary API replicas exist. This is deliberate containment, not the
/// long-term fix: it does not let relay itself scale past one replica's
/// throughput, and it does not solve presence/forwarding across multiple
/// relay nodes. The deferred long-term option -- shared presence plus
/// inter-node pub/sub (Redis or NATS) so relay can itself run more than one
/// replica -- is recorded as explicit future work in `DECISIONS.md`, not
/// built here. Moving `RelayRegistry` into Postgres alone would not have
/// been sufficient: it would fix presence *discovery* but not actual
/// cross-process WebSocket frame forwarding, which needs a real message bus
/// between nodes.
#[derive(Clone, Default)]
pub struct RelayRegistry {
    peers: Arc<RwLock<HashMap<uuid::Uuid, ConnectedReplica>>>,
    /// Relay tickets (H4(b)) already redeemed, keyed by `jti`, mapped to
    /// their own `exp` so expired entries can be swept cheaply. In-memory
    /// is the *correct* choice here, not merely convenient: H3 guarantees
    /// exactly one relay process ever exists, so there is no cross-replica
    /// redemption race to guard against the way there would be for
    /// anything else in this codebase backed by `Arc<RwLock<HashSet<_>>>`
    /// (see the H2 fix this same session for the general case). A ticket
    /// minted on any ordinary API replica is only ever redeemed here.
    redeemed_tickets: Arc<RwLock<HashMap<uuid::Uuid, u64>>>,
}

impl RelayRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically checks-and-marks a ticket `jti` as redeemed. Returns
    /// `true` the first time a given `jti` is presented, `false` on every
    /// subsequent attempt -- the actual single-use enforcement. Sweeps
    /// entries whose `exp` has already passed on every call, so this
    /// cannot grow without bound over a long-lived process: it never
    /// holds more entries than tickets issued within one TTL window.
    async fn redeem_ticket_once(&self, jti: uuid::Uuid, exp: u64) -> bool {
        let mut redeemed = self.redeemed_tickets.write().await;
        redeemed.retain(|_, other_exp| *other_exp > unix_seconds());
        if redeemed.contains_key(&jti) {
            return false;
        }
        redeemed.insert(jti, exp);
        true
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

/// Relay tickets' `TokenClaims::token_type` discriminator, distinct from
/// `"access"`/`"refresh"` so a normal access/refresh token can never be
/// presented where a ticket is expected, or vice versa.
const RELAY_TICKET_TOKEN_TYPE: &str = "relay_ticket";

/// How long a minted relay ticket remains valid. Short by design: a ticket
/// only needs to survive the time between minting it over HTTPS and
/// completing the WebSocket upgrade that follows immediately after, over a
/// connection the client already has open. 30s comfortably covers real
/// network latency (including a retry) without leaving a meaningfully
/// long-lived credential sitting in a URL if one did leak into a log.
const RELAY_TICKET_TTL_SECONDS: u64 = 30;

#[derive(Debug, Deserialize)]
pub struct IssueTicketRequest {
    pub target_id: String,
    /// The replica identity the caller intends to register as once it
    /// dials the WebSocket with the minted ticket. Checked against
    /// `replica_ownership` here (H7) rather than trusted as the
    /// unauthenticated `self` query parameter it used to be — see
    /// `claim_replica_ownership` and this fn's "Ticket design" doc comment.
    pub self_replica: String,
}

/// First-claim-wins ownership check for a relay replica identity, backing
/// audit finding H7. `INSERT ... ON CONFLICT DO NOTHING` followed by a
/// `SELECT` relies on the table's own primary key to resolve concurrent
/// first claims atomically — no `SELECT`-then-`INSERT` race window.
///
/// Returns `Ok(())` if `replica_id` is unclaimed (and claims it for
/// `user_id` now) or already claimed by this same `user_id`. Returns
/// `Err(())` if it is already claimed by a different user — the caller
/// must refuse to mint a ticket in that case.
///
/// Known, disclosed limitation (see DECISIONS.md): this is permanent
/// per-user, so a second real employee logging into the same shared
/// physical install (which legitimately reuses one persisted `ReplicaId`
/// per desktop-shell's own design, see `SessionInfo::from_session`) will
/// be refused a ticket for it once a first employee has claimed it. That
/// is an availability inconvenience for an intentionally narrow scenario,
/// not a reopening of the vulnerability this table closes.
async fn claim_replica_ownership(
    pool: &ProjectionPool,
    replica_id: uuid::Uuid,
    user_id: uuid::Uuid,
    organization_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    let now = unix_seconds() as i64;
    match pool {
        ProjectionPool::Sqlite(pool) => {
            sqlx::query(
                "INSERT INTO replica_ownership (replica_id, user_id, organization_id, claimed_at) \
                 VALUES (?, ?, ?, ?) ON CONFLICT (replica_id) DO NOTHING",
            )
            .bind(replica_id.as_bytes().to_vec())
            .bind(user_id.as_bytes().to_vec())
            .bind(organization_id.as_bytes().to_vec())
            .bind(now)
            .execute(pool)
            .await?;

            let owner: (Vec<u8>,) =
                sqlx::query_as("SELECT user_id FROM replica_ownership WHERE replica_id = ?")
                    .bind(replica_id.as_bytes().to_vec())
                    .fetch_one(pool)
                    .await?;
            if owner.0 == user_id.as_bytes().to_vec() {
                Ok(())
            } else {
                Err(sqlx::Error::RowNotFound)
            }
        }
        ProjectionPool::Postgres(pool) => {
            sqlx::query(
                "INSERT INTO replica_ownership (replica_id, user_id, organization_id, claimed_at) \
                 VALUES ($1, $2, $3, $4) ON CONFLICT (replica_id) DO NOTHING",
            )
            .bind(replica_id)
            .bind(user_id)
            .bind(organization_id)
            .bind(now)
            .execute(pool)
            .await?;

            let owner: (uuid::Uuid,) =
                sqlx::query_as("SELECT user_id FROM replica_ownership WHERE replica_id = $1")
                    .bind(replica_id)
                    .fetch_one(pool)
                    .await?;
            if owner.0 == user_id {
                Ok(())
            } else {
                Err(sqlx::Error::RowNotFound)
            }
        }
    }
}

#[derive(Debug, Serialize)]
pub struct IssueTicketResponse {
    pub ticket: String,
    pub expires_in: u64,
}

/// `POST /api/relay-ticket` — mints a short-lived, single-use, target-scoped
/// relay ticket (audit finding H4(b) item 2) for an already-authenticated
/// caller. Deliberately a normal, stateless, horizontally-scalable HTTP
/// route: it reads nothing but the shared JWT signing key every ordinary
/// API replica already has, so — unlike the WebSocket upgrade itself — it
/// does not need to land on the single dedicated relay replica (see
/// `routes::relay`'s own module doc comment and this route's Ingress
/// placement in `deploy/helm/onyx-api-relay`, which deliberately excludes
/// this path).
///
/// # Ticket design
/// - **Lifetime**: `RELAY_TICKET_TTL_SECONDS` (30s) from issuance, enforced
///   by `validate_token`'s ordinary `exp` check — the same mechanism access
///   and refresh tokens already use, not a parallel one.
/// - **Scope**: bound to the specific `target_id` the caller declared when
///   requesting it (`TokenScope::object_id`); `relay_route` rejects a
///   ticket presented against any other path segment. A ticket minted to
///   reach replica A cannot be replayed against replica B.
/// - **Single-use**: enforced server-side by `RelayRegistry::redeem_ticket_once`
///   keyed on the ticket's own `jti`, checked the moment the WebSocket
///   upgrade is accepted (see `relay_route`). Presenting the same ticket
///   twice — whether replayed by an attacker who intercepted it, or by a
///   buggy client retry — succeeds at most once.
/// - **Self-identity binding (H7)**: also bound to the specific
///   `self_replica` the caller declared, via `TokenScope::self_replica`.
///   Minting refuses (401) unless `claim_replica_ownership` confirms the
///   caller's `user_id` owns that replica id (first-claim-wins, durable in
///   `replica_ownership`). `relay_route` then requires the WebSocket's
///   `self` query parameter to match this bound claim exactly, rather than
///   trusting that unauthenticated query string directly — closing the
///   same-tenant replica-impersonation/connection-displacement gap the
///   original design left open (see DECISIONS.md).
pub async fn issue_ticket(
    State(state): State<ApiState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<IssueTicketRequest>,
) -> Result<Json<IssueTicketResponse>, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let actor = authenticate_headers(&state, &headers).await?;
    let target_id = uuid::Uuid::parse_str(&payload.target_id).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_REPLICA_ID",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation_id.clone(),
            serde_json::json!({ "message": "target_id must be a replica UUID" }),
        )
    })?;
    let self_replica_id = uuid::Uuid::parse_str(&payload.self_replica).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "INVALID_REPLICA_ID",
            "VALIDATION",
            "NON_RETRYABLE",
            correlation_id.clone(),
            serde_json::json!({ "message": "self_replica must be a replica UUID" }),
        )
    })?;

    let actor_user_id = uuid::Uuid::parse_str(&actor.user_id)
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?;
    let actor_org_id = uuid::Uuid::parse_str(&actor.organization_id)
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?;
    claim_replica_ownership(
        &state.projection_pool,
        self_replica_id,
        actor_user_id,
        actor_org_id,
    )
    .await
    .map_err(|_| {
        tracing::warn!(
            organization_id = %actor.organization_id,
            self_replica = %self_replica_id,
            "relay: refused to mint a ticket for a replica id the caller does not own"
        );
        ApiError::unauthorized(correlation_id.clone())
    })?;

    let now = unix_seconds();
    let claims = TokenClaims {
        sub: actor.user_id,
        username: actor.username,
        organization_id: actor.organization_id,
        token_type: RELAY_TICKET_TOKEN_TYPE.to_string(),
        // Propagated from the caller's own authenticated session so a
        // relay ticket cannot claim a different class than the session
        // that minted it. Out of this task's scope to add a dedicated
        // capability check on this endpoint (relay/sync participation
        // isn't in ONYX-MOB-01 §9's enumerated mutation list -- see
        // DECISIONS.md's H10 entry), but propagating the real value
        // here is a one-line correctness fix, not new enforcement.
        client_type: actor.client_type,
        scope: TokenScope {
            object_type: "relay".to_string(),
            object_id: Some(target_id.to_string()),
            command_types: Vec::new(),
            delegation_depth: 0,
            self_replica: Some(self_replica_id.to_string()),
        },
        iat: now,
        exp: now + RELAY_TICKET_TTL_SECONDS,
        jti: uuid::Uuid::new_v4().to_string(),
    };
    let secret = state
        .secret_provider
        .get("ONYX_AUTHORITY_SIGNING_KEY")
        .await
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?;
    let ticket = Ed25519JwtCodec::from_rotating_secret(&secret)
        .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?
        .encode(&claims)
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "TOKEN_ISSUANCE_FAILED",
                "INFRASTRUCTURE",
                "TRANSIENT",
                correlation_id,
                serde_json::json!({}),
            )
        })?;

    Ok(Json(IssueTicketResponse {
        ticket,
        expires_in: RELAY_TICKET_TTL_SECONDS,
    }))
}

#[derive(Debug, Deserialize)]
pub struct RelayAuth {
    pub ticket: String,
    /// The dialling replica's own id.
    ///
    /// Required, and deliberately not inferred from the first frame. Lazy
    /// registration looked simpler but is wrong twice over: a replica that
    /// only ever listens would never become addressable at all, and even
    /// between two replicas that both transmit there is a race where the
    /// first frame arrives before its target has finished registering and is
    /// silently dropped. Declaring identity in the handshake makes a
    /// connection addressable the moment it is open.
    ///
    /// H7: this is still a plain, unauthenticated query parameter — it is
    /// not itself trusted. `relay_route` requires it to match the
    /// `self_replica` claim bound into the ticket at mint time (see
    /// `issue_ticket`'s "Ticket design" doc comment); a mismatch is
    /// rejected before the WebSocket upgrade is ever accepted.
    #[serde(rename = "self")]
    pub self_replica: String,
}

/// `GET /api/relay/:target_id?ticket=...`
///
/// The path names the peer this connection wants to reach, matching the URL
/// `CloudRelayTransport::connect` builds. The caller's own identity is not in
/// the path; it is taken from the `sender_replica` field of the first frame
/// it sends (see `serve_relay`).
///
/// Authenticates with a relay ticket (`issue_ticket`), not a raw access
/// token (H4(b)) — see `RelayAuth`'s and `issue_ticket`'s doc comments.
pub async fn relay_route(
    ws: WebSocketUpgrade,
    Path(target_id): Path<String>,
    Query(params): Query<RelayAuth>,
    State(state): State<ApiState>,
) -> Result<Response, ApiError> {
    let correlation_id = uuid::Uuid::new_v4().to_string();
    let claims = validate_token(&state, &params.ticket, RELAY_TICKET_TOKEN_TYPE).await?;

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

    // Scope check: this ticket must have been minted for exactly this
    // target. A ticket minted to reach replica A must not work against
    // any other path segment, including a legitimate, still-live ticket
    // simply retargeted by an attacker who intercepted the URL.
    if claims.scope.object_id.as_deref() != Some(default_target.to_string().as_str()) {
        return Err(ApiError::unauthorized(correlation_id));
    }

    // H7: the `self` query parameter is not authenticated by itself — it
    // must match the replica identity the ticket was actually minted for
    // (verified against `replica_ownership` at mint time). Without this
    // check an attacker could present a valid ticket for `target_id` while
    // declaring an arbitrary, un-owned `self_replica` in the URL, since
    // this parameter is otherwise never cross-checked against anything.
    if claims.scope.self_replica.as_deref() != Some(self_replica.to_string().as_str()) {
        tracing::warn!(
            organization_id = %claims.organization_id,
            declared_self = %self_replica,
            "relay: rejected a ticket whose bound self_replica does not match the connection's declared identity"
        );
        return Err(ApiError::unauthorized(correlation_id));
    }

    // Single-use enforcement (H4(b)): this jti must never have been
    // redeemed before. Safe as process-local, in-memory state specifically
    // because H3 guarantees this handler only ever runs in one process —
    // see `RelayRegistry::redeem_ticket_once`'s own doc comment.
    if !state
        .relay_registry
        .redeem_ticket_once(
            uuid::Uuid::parse_str(&claims.jti)
                .map_err(|_| ApiError::unauthorized(correlation_id.clone()))?,
            claims.exp,
        )
        .await
    {
        tracing::warn!(
            organization_id = %claims.organization_id,
            "relay: rejected a relay ticket that was already redeemed"
        );
        return Err(ApiError::unauthorized(correlation_id));
    }

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
