//! Organization-scoped peer discovery. Frozen contract per Team Prompt 4
//! §3.4, built against real `platform_kernel::{OrganizationId, ReplicaId,
//! Timestamp}`.

use crate::placeholder_types::DiscoveryError;
use async_trait::async_trait;
use platform_kernel::{OrganizationId, ReplicaId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;

/// Discovers peers in the same organization.
/// Combines local discovery (Wi-Fi Direct mDNS, BLE advertisement)
/// and cloud presence (via Cloud Relay presence endpoint).
#[async_trait]
pub trait Discovery: Send + Sync {
    /// Discover peers in the same organization.
    /// Filters out any peer with a different organization_id.
    async fn discover_peers(
        &self,
        organization_id: OrganizationId,
    ) -> Result<Vec<PeerInfo>, DiscoveryError>;
}

/// A peer discovered on the network.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: ReplicaId,
    pub organization_id: OrganizationId,
    pub endpoints: Vec<SocketAddr>,        // For QUIC cross-network
    pub service_name: Option<String>,      // For Wi-Fi Direct / mDNS
    pub mac_address: Option<String>,       // For Wi-Fi Direct (mobile)
    pub bluetooth_address: Option<String>, // For BLE
    pub last_seen: Timestamp,
    /// Marks a peer as sourced from cloud presence rather than local radio
    /// discovery. Used by `CompositeDiscovery::discover` to prefer cloud
    /// endpoints on dedup. Not part of the literal prompt's `PeerInfo`
    /// struct — see DECISIONS.md for why it was added.
    #[serde(default)]
    pub from_cloud: bool,
}

impl PeerInfo {
    #[cfg(any(test, feature = "test-support"))]
    pub fn dummy() -> Self {
        Self {
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

    #[cfg(any(test, feature = "test-support"))]
    pub fn dummy_no_endpoints() -> Self {
        let mut p = Self::dummy();
        p.endpoints = vec![];
        p
    }
}

/// Combines local and cloud discovery.
///
/// Holds `Discovery` trait objects rather than the two concrete placeholder
/// types it was first written against. Those types are stubs, so naming them
/// in the field types made the merge unreachable for any real implementation:
/// a composition root could not substitute `lan-discovery`'s UDP-broadcast
/// scanner without editing this struct.
pub struct CompositeDiscovery {
    cloud_discovery: Box<dyn Discovery>,
    local_discovery: Box<dyn Discovery>,
}

impl CompositeDiscovery {
    pub fn new(cloud: impl Discovery + 'static, local: impl Discovery + 'static) -> Self {
        Self {
            cloud_discovery: Box::new(cloud),
            local_discovery: Box::new(local),
        }
    }

    /// For a composition root that already owns its discovery as an `Arc`.
    pub fn from_boxed(cloud: Box<dyn Discovery>, local: Box<dyn Discovery>) -> Self {
        Self {
            cloud_discovery: cloud,
            local_discovery: local,
        }
    }

    /// Discover peers, merging local and cloud results.
    ///
    /// Dedup strategy: keyed by `ReplicaId` via a `HashMap`, with
    /// cloud-sourced entries taking precedence over local-sourced entries
    /// for the same peer ID (cloud entries carry resolvable `endpoints`
    /// needed for QUIC; local entries may not). The prompt's literal
    /// sample (`peers.dedup_by(|a, b| a.id == b.id)`) only removes
    /// *adjacent* duplicates in a `Vec` and cannot express "cloud wins" —
    /// see DECISIONS.md.
    pub async fn discover(&self, org_id: OrganizationId) -> Vec<PeerInfo> {
        let mut by_id: std::collections::HashMap<ReplicaId, PeerInfo> =
            std::collections::HashMap::new();

        // 1. Local discovery (Wi-Fi Direct, BLE) — inserted first.
        if let Ok(local) = self.local_discovery.discover_peers(org_id).await {
            for peer in local {
                by_id.insert(peer.id, peer);
            }
        }

        // 2. Cloud presence — overwrites local entries with the same ID.
        if let Ok(cloud) = self.cloud_discovery.discover_peers(org_id).await {
            for mut peer in cloud {
                peer.from_cloud = true;
                by_id.insert(peer.id, peer);
            }
        }

        let _seen_ids: HashSet<ReplicaId> = by_id.keys().copied().collect();
        by_id.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::placeholder_types::{CloudDiscovery, LocalDiscovery};

    #[tokio::test]
    async fn discovery_filters_by_organization() {
        let org_a = OrganizationId::new_random();
        let org_b = OrganizationId::new_random();

        let mut peer_in_a = PeerInfo::dummy();
        peer_in_a.organization_id = org_a;
        let mut peer_in_b = PeerInfo::dummy();
        peer_in_b.organization_id = org_b;

        let local = LocalDiscovery::with_peers(vec![peer_in_a.clone(), peer_in_b.clone()]);
        let cloud = CloudDiscovery::with_peers(vec![]);

        let composite = CompositeDiscovery::new(cloud, local);
        let result = composite.discover(org_a).await;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, peer_in_a.id);
    }

    #[tokio::test]
    async fn no_cross_org_discovery() {
        let org_a = OrganizationId::new_random();
        let org_b = OrganizationId::new_random();

        let mut peer_in_b = PeerInfo::dummy();
        peer_in_b.organization_id = org_b;

        let local = LocalDiscovery::with_peers(vec![peer_in_b.clone()]);
        let cloud = CloudDiscovery::with_peers(vec![peer_in_b]);

        let composite = CompositeDiscovery::new(cloud, local);
        let result = composite.discover(org_a).await;

        assert!(
            result.is_empty(),
            "no peer from org_b should appear when querying org_a"
        );
    }

    #[tokio::test]
    async fn cloud_peer_takes_precedence_on_dedup() {
        let org = OrganizationId::new_random();
        let replica = ReplicaId::new_random();

        let mut local_version = PeerInfo::dummy();
        local_version.id = replica;
        local_version.organization_id = org;
        local_version.endpoints = vec![];

        let mut cloud_version = PeerInfo::dummy();
        cloud_version.id = replica;
        cloud_version.organization_id = org;
        cloud_version.endpoints = vec!["10.0.0.1:4433".parse().unwrap()];

        let local = LocalDiscovery::with_peers(vec![local_version]);
        let cloud = CloudDiscovery::with_peers(vec![cloud_version]);

        let composite = CompositeDiscovery::new(cloud, local);
        let result = composite.discover(org).await;

        assert_eq!(result.len(), 1);
        assert!(result[0].from_cloud);
        assert_eq!(result[0].endpoints.len(), 1);
    }
}
