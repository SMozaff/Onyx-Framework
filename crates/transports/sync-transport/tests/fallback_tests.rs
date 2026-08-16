//! Team Prompt 4 §9.1 acceptance test, plus the permission-denial variant
//! from the acceptance criteria table (§11).

use std::time::Duration;
use sync_transport::test_support::MockTransport;
use sync_transport::{PeerInfo, TransportSelector, TransportType};

#[tokio::test]
async fn selector_falls_back_through_all_transports() {
    let wifi = MockTransport::new(TransportType::WifiDirect, true, true);
    let ble = MockTransport::new(TransportType::BluetoothLE, true, true);
    let quic = MockTransport::new(TransportType::QuicCrossNetwork, true, true);
    let cloud = MockTransport::new(TransportType::CloudRelay, true, false);

    let selector = TransportSelector::new(
        Some(Box::new(wifi)),
        Some(Box::new(ble)),
        Some(Box::new(quic)),
        Box::new(cloud),
    );
    let conn = selector
        .connect_best(PeerInfo::dummy(), Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(conn.transport_type(), TransportType::CloudRelay);
}

#[tokio::test]
async fn fallback_on_permission_denied() {
    // Permission denial is modeled as is_available() == false, per §6.3.
    let wifi = MockTransport::new(TransportType::WifiDirect, false, false);
    let ble = MockTransport::new(TransportType::BluetoothLE, false, false);
    let quic = MockTransport::new(TransportType::QuicCrossNetwork, false, false);
    let cloud = MockTransport::new(TransportType::CloudRelay, true, false);

    let selector = TransportSelector::new(
        Some(Box::new(wifi)),
        Some(Box::new(ble)),
        Some(Box::new(quic)),
        Box::new(cloud),
    );
    let conn = selector
        .connect_best(PeerInfo::dummy(), Duration::from_secs(1))
        .await
        .unwrap();

    assert_eq!(conn.transport_type(), TransportType::CloudRelay);
}
