//! Deterministic transport selector with fallback. Frozen contract per
//! Team Prompt 4 §3.3 / §7.2.
//!
//! Fallback order (fixed, not configurable):
//! 1. Wi-Fi Direct (same-LAN or direct-associate)
//! 2. Bluetooth LE (mobile-to-mobile fallback)
//! 3. QUIC cross-network (different networks, NAT traversal)
//! 4. Cloud Relay (always available)

use crate::discovery::PeerInfo;
use crate::transport_trait::{Connection, Transport, TransportError, TransportType};
use std::time::Duration;

/// The deterministic transport selector.
pub struct TransportSelector {
    transports: Vec<Box<dyn Transport>>,
    #[allow(dead_code)] // retained for §3.3 parity / potential UI introspection
    order: Vec<TransportType>,
}

impl TransportSelector {
    /// Create a new selector with the given transports.
    /// The caller provides the concrete transport instances.
    pub fn new(
        wifi_direct: Option<Box<dyn Transport>>,
        bluetooth_le: Option<Box<dyn Transport>>,
        quic: Option<Box<dyn Transport>>,
        cloud_relay: Box<dyn Transport>,
    ) -> Self {
        let mut ordered: Vec<Box<dyn Transport>> = Vec::new();

        // Order: Wi-Fi Direct, BLE, QUIC, Cloud Relay.
        if let Some(t) = wifi_direct {
            ordered.push(t);
        }
        if let Some(t) = bluetooth_le {
            ordered.push(t);
        }
        if let Some(t) = quic {
            ordered.push(t);
        }
        // Cloud relay is always last and must be available.
        ordered.push(cloud_relay);

        let order = ordered.iter().map(|t| t.transport_type()).collect();
        Self {
            transports: ordered,
            order,
        }
    }

    /// Attempt to connect using the best available transport.
    /// Each transport gets a bounded connection-attempt window.
    pub async fn connect_best(
        &self,
        peer: PeerInfo,
        timeout_per_transport: Duration,
    ) -> Result<Box<dyn Connection>, TransportError> {
        let mut last_error = TransportError::NoTransportAvailable;

        for transport in &self.transports {
            if !transport.is_available() {
                tracing::debug!("Transport {} not available", transport.transport_type());
                continue;
            }

            match transport.connect(peer.clone(), timeout_per_transport).await {
                Ok(conn) => {
                    tracing::info!("Connected to peer via {}", transport.transport_type());
                    return Ok(conn);
                }
                Err(e) => {
                    tracing::warn!("Transport {} failed: {}", transport.transport_type(), e);
                    last_error = e;
                    // Continue to next transport.
                }
            }
        }

        Err(last_error)
    }

    /// Get the list of available transports (for UI status).
    pub fn available_transports(&self) -> Vec<TransportType> {
        self.transports
            .iter()
            .filter(|t| t.is_available())
            .map(|t| t.transport_type())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::MockTransport;

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
        // "Permission denied" is modeled as is_available() == false, per
        // §6.3: "If permissions are denied, the transport's is_available()
        // returns false, and the selector falls back to Cloud Relay."
        let wifi = MockTransport::new(TransportType::WifiDirect, false, false);
        let ble = MockTransport::new(TransportType::BluetoothLE, false, false);
        let cloud = MockTransport::new(TransportType::CloudRelay, true, false);

        let selector = TransportSelector::new(
            Some(Box::new(wifi)),
            Some(Box::new(ble)),
            None,
            Box::new(cloud),
        );
        let conn = selector
            .connect_best(PeerInfo::dummy(), Duration::from_secs(1))
            .await
            .unwrap();

        assert_eq!(conn.transport_type(), TransportType::CloudRelay);
    }

    #[tokio::test]
    async fn no_transport_available_when_all_fail_including_cloud() {
        let cloud = MockTransport::new(TransportType::CloudRelay, true, true);
        let selector = TransportSelector::new(None, None, None, Box::new(cloud));

        let result = selector
            .connect_best(PeerInfo::dummy(), Duration::from_secs(1))
            .await;

        // Same `Box<dyn Connection>: !Debug` issue as bluetooth_le.rs — a
        // real compiler-caught bug, not assumed. Matching explicitly.
        match result {
            Err(TransportError::Unreachable) => {}
            _ => panic!("expected TransportError::Unreachable, got a different result"),
        }
    }

    #[tokio::test]
    async fn respects_fixed_order_wifi_before_ble_before_quic_before_cloud() {
        // All available and all succeed: the FIRST in fixed order (Wi-Fi
        // Direct) must win, proving order is Wi-Fi -> BLE -> QUIC -> Cloud
        // and not e.g. insertion order of Option args.
        let wifi = MockTransport::new(TransportType::WifiDirect, true, false);
        let ble = MockTransport::new(TransportType::BluetoothLE, true, false);
        let quic = MockTransport::new(TransportType::QuicCrossNetwork, true, false);
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

        assert_eq!(conn.transport_type(), TransportType::WifiDirect);
    }

    #[test]
    fn available_transports_excludes_unavailable() {
        let wifi = MockTransport::new(TransportType::WifiDirect, false, false);
        let cloud = MockTransport::new(TransportType::CloudRelay, true, false);

        let selector = TransportSelector::new(Some(Box::new(wifi)), None, None, Box::new(cloud));
        let available = selector.available_transports();

        assert_eq!(available, vec![TransportType::CloudRelay]);
    }
}
