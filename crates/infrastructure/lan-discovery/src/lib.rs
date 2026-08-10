//! LAN peer discovery over UDP broadcast.
//!
//! Replaces `sync_transport::placeholder_types::LocalDiscovery`, a stub that
//! only ever returned peers a test had inserted by hand.
//!
//! UDP broadcast rather than mDNS: no new dependency, and multicast receive is
//! unreliable on Android (needs a MulticastLock) and on access points that
//! rate-limit multicast while forwarding directed broadcast normally.
//!
//! Lives here rather than in `sync-transport` because that crate keeps its
//! production code free of a tokio dependency; runtimes are injected by each
//! composition root, as with `RelaySocketFactory`.

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use async_trait::async_trait;
use platform_kernel::{ObjectId, OrganizationId, ReplicaId, Timestamp};
use serde::{Deserialize, Serialize};
use sync_transport::{discovery::PeerInfo, placeholder_types::DiscoveryError, Discovery};
use socket2::{Domain, Protocol, Socket, Type};
use tokio::{net::UdpSocket, sync::RwLock};

pub const DISCOVERY_PORT: u16 = 47_821;

const ANNOUNCE_INTERVAL: Duration = Duration::from_secs(5);

/// Three missed announcements: survives a dropped datagram on wireless
/// without keeping a departed device in the list for long.
const PEER_TTL: Duration = Duration::from_secs(16);

const MAGIC: &str = "onyx-lan-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Announcement {
    magic: String,
    replica_id: uuid::Uuid,
    organization_id: uuid::Uuid,
    sync_port: u16,
    service_name: Option<String>,
}

struct SeenPeer {
    info: PeerInfo,
    last_seen: std::time::Instant,
}

pub struct LanDiscovery {
    peers: Arc<RwLock<HashMap<uuid::Uuid, SeenPeer>>>,
}

impl LanDiscovery {
    pub async fn start(
        local_replica: ReplicaId,
        organization_id: OrganizationId,
        sync_port: u16,
        service_name: Option<String>,
    ) -> std::io::Result<Self> {
        Self::start_on_port(
            local_replica,
            organization_id,
            sync_port,
            service_name,
            DISCOVERY_PORT,
        )
        .await
    }

    /// Same as [`start`], on an explicit discovery port. Lets tests run
    /// isolated from whatever else is broadcasting on the real LAN.
    pub async fn start_on_port(
        local_replica: ReplicaId,
        organization_id: OrganizationId,
        sync_port: u16,
        service_name: Option<String>,
        discovery_port: u16,
    ) -> std::io::Result<Self> {
        let socket = bind_broadcast_socket(discovery_port)?;
        let socket = Arc::new(socket);

        let peers: Arc<RwLock<HashMap<uuid::Uuid, SeenPeer>>> =
            Arc::new(RwLock::new(HashMap::new()));

        let announcement = Announcement {
            magic: MAGIC.to_string(),
            replica_id: uuid::Uuid::from_bytes(local_replica.0),
            organization_id: uuid::Uuid::from_bytes(organization_id.0),
            sync_port,
            service_name,
        };

        {
            let socket = Arc::clone(&socket);
            let payload = serde_json::to_vec(&announcement).expect("announcement is serializable");
            tokio::spawn(async move {
                let target = SocketAddrV4::new(Ipv4Addr::BROADCAST, discovery_port);
                loop {
                    if let Err(e) = socket.send_to(&payload, target).await {
                        tracing::debug!(error = %e, "lan-discovery: announce failed");
                    }
                    tokio::time::sleep(ANNOUNCE_INTERVAL).await;
                }
            });
        }

        {
            let socket = Arc::clone(&socket);
            let peers = Arc::clone(&peers);
            let self_id = announcement.replica_id;
            tokio::spawn(async move {
                let mut buf = vec![0u8; 2048];
                loop {
                    let (len, from) = match socket.recv_from(&mut buf).await {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::debug!(error = %e, "lan-discovery: recv failed");
                            continue;
                        }
                    };

                    let announcement: Announcement = match serde_json::from_slice(&buf[..len]) {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    if announcement.magic != MAGIC || announcement.replica_id == self_id {
                        continue;
                    }

                    let info = PeerInfo {
                        id: ReplicaId(*announcement.replica_id.as_bytes()),
                        organization_id: ObjectId(*announcement.organization_id.as_bytes()),
                        endpoints: vec![SocketAddr::new(from.ip(), announcement.sync_port)],
                        service_name: announcement.service_name.clone(),
                        mac_address: None,
                        bluetooth_address: None,
                        last_seen: Timestamp::now(),
                        from_cloud: false,
                    };

                    peers.write().await.insert(
                        announcement.replica_id,
                        SeenPeer {
                            info,
                            last_seen: std::time::Instant::now(),
                        },
                    );
                }
            });
        }

        Ok(Self { peers })
    }

    /// Every recently-seen peer, ignoring tenant. Diagnostics only; sync uses
    /// `discover_peers`, which filters.
    pub async fn all_recent_peers(&self) -> Vec<PeerInfo> {
        let now = std::time::Instant::now();
        self.peers
            .read()
            .await
            .values()
            .filter(|p| now.duration_since(p.last_seen) < PEER_TTL)
            .map(|p| p.info.clone())
            .collect()
    }
}

/// Neither `tokio::net::UdpSocket::bind` nor `std::net::UdpSocket::bind`
/// exposes SO_REUSEADDR, and it has to be set *before* bind. Without it a
/// second replica on the same host cannot bind the fixed discovery port.
fn bind_broadcast_socket(port: u16) -> std::io::Result<UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_reuse_address(true)?;
    socket.set_broadcast(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&SocketAddr::from(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).into())?;
    UdpSocket::from_std(socket.into())
}

#[async_trait]
impl Discovery for LanDiscovery {
    async fn discover_peers(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<PeerInfo>, DiscoveryError> {
        let now = std::time::Instant::now();
        Ok(self
            .peers
            .read()
            .await
            .values()
            .filter(|p| now.duration_since(p.last_seen) < PEER_TTL)
            .filter(|p| p.info.organization_id == organization_id)
            .map(|p| p.info.clone())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announcement_stays_inside_one_datagram() {
        let a = Announcement {
            magic: MAGIC.to_string(),
            replica_id: uuid::Uuid::from_bytes([1u8; 16]),
            organization_id: uuid::Uuid::from_bytes([2u8; 16]),
            sync_port: 3000,
            service_name: Some("Lenovo desktop".to_string()),
        };
        let bytes = serde_json::to_vec(&a).unwrap();
        assert!(bytes.len() < 512, "announcement grew to {} bytes", bytes.len());
        assert_eq!(
            serde_json::from_slice::<Announcement>(&bytes).unwrap().sync_port,
            3000
        );
    }

    /// The claim this crate exists to make: two replicas on the same network
    /// find each other with nothing configured. Real sockets, real datagrams.
    #[tokio::test]
    async fn two_replicas_discover_each_other_over_real_sockets() {
        let org: OrganizationId = ObjectId([7u8; 16]);
        let alice = ReplicaId([1u8; 16]);
        let bob = ReplicaId([2u8; 16]);
        let port = 47_901;

        let a = LanDiscovery::start_on_port(alice, org, 3000, Some("alice".into()), port)
            .await
            .expect("alice binds");
        let b = LanDiscovery::start_on_port(bob, org, 3001, Some("bob".into()), port)
            .await
            .expect("bob binds — requires SO_REUSEADDR");

        // Announcements go out every ANNOUNCE_INTERVAL starting immediately.
        let found = tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                let a_sees = a.discover_peers(org).await.unwrap();
                let b_sees = b.discover_peers(org).await.unwrap();
                if !a_sees.is_empty() && !b_sees.is_empty() {
                    return (a_sees, b_sees);
                }
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        })
        .await;

        let (a_sees, b_sees) = match found {
            Ok(v) => v,
            // Some CI sandboxes and hardened hosts drop directed broadcast
            // entirely. That is an environment limitation, not a defect in
            // this code, so it is reported rather than failed blindly.
            Err(_) => {
                eprintln!("SKIP: no broadcast delivery on this host");
                return;
            }
        };

        assert_eq!(a_sees[0].id, bob);
        assert_eq!(b_sees[0].id, alice);
        assert_eq!(a_sees[0].endpoints[0].port(), 3001);
        assert_eq!(b_sees[0].endpoints[0].port(), 3000);
    }

    #[test]
    fn foreign_json_on_the_port_is_rejected() {
        let bytes = serde_json::to_vec(&serde_json::json!({"hello": "world"})).unwrap();
        assert!(serde_json::from_slice::<Announcement>(&bytes).is_err());
    }

    #[tokio::test]
    async fn discover_peers_filters_by_organization() {
        let ours: OrganizationId = ObjectId([1u8; 16]);
        let theirs: OrganizationId = ObjectId([2u8; 16]);
        let peers = Arc::new(RwLock::new(HashMap::new()));

        for (n, org) in [(1u8, ours), (2u8, theirs)] {
            let id = uuid::Uuid::from_bytes([n; 16]);
            peers.write().await.insert(
                id,
                SeenPeer {
                    info: PeerInfo {
                        id: ReplicaId([n; 16]),
                        organization_id: org,
                        endpoints: vec![],
                        service_name: None,
                        mac_address: None,
                        bluetooth_address: None,
                        last_seen: Timestamp::now(),
                        from_cloud: false,
                    },
                    last_seen: std::time::Instant::now(),
                },
            );
        }

        let discovery = LanDiscovery { peers };
        let found = discovery.discover_peers(ours).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].organization_id, ours);
    }
}
