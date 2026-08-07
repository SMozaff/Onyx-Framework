//! Team Prompt 4 §9.4 and the "No Cross-Org Leak" acceptance criterion
//! (§11: `discovery_tests::no_cross_org_discovery`). This file wasn't in
//! the §2 file manifest table, but is named explicitly by both §9.4 and
//! the acceptance criteria table, so it's included as its own integration
//! test binary rather than folded into the unit tests already in
//! `discovery.rs`. See DECISIONS.md.

use platform_kernel::OrganizationId;
use sync_transport::placeholder_types::{CloudDiscovery, LocalDiscovery};
use sync_transport::{CompositeDiscovery, PeerInfo};

#[tokio::test]
async fn discovery_filters_by_organization() {
    let org_a = OrganizationId::new_random();
    let org_b = OrganizationId::new_random();

    let mut peer_a = PeerInfo::dummy();
    peer_a.organization_id = org_a;
    let mut peer_b = PeerInfo::dummy();
    peer_b.organization_id = org_b;

    let local = LocalDiscovery::with_peers(vec![peer_a.clone(), peer_b.clone()]);
    let cloud = CloudDiscovery::with_peers(vec![]);
    let composite = CompositeDiscovery::new(cloud, local);

    let result = composite.discover(org_a).await;

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].id, peer_a.id);
}

#[tokio::test]
async fn no_cross_org_discovery() {
    let org_a = OrganizationId::new_random();
    let org_b = OrganizationId::new_random();

    let mut peer_b = PeerInfo::dummy();
    peer_b.organization_id = org_b;

    let local = LocalDiscovery::with_peers(vec![peer_b.clone()]);
    let cloud = CloudDiscovery::with_peers(vec![peer_b]);
    let composite = CompositeDiscovery::new(cloud, local);

    let result = composite.discover(org_a).await;

    assert!(
        result.is_empty(),
        "discovery must never return a peer outside the queried organization"
    );
}
