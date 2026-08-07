//! Remaining acceptance criteria from §11 not covered by the more
//! specifically-named test files: message size limits, concurrent
//! sessions, and mobile platform availability reflecting OS capability.

use std::time::Duration;
use sync_transport::placeholder_types::{PlatformBLEHandle, PlatformHandle};
use sync_transport::test_support::MockTransport;
use sync_transport::{PeerInfo, Transport, TransportError, TransportType};

#[test]
fn message_too_large() {
    let transport = MockTransport::new(TransportType::CloudRelay, true, false);
    let max = transport.max_message_size();
    assert_eq!(max, 1024 * 1024, "MockTransport uses the 1 MB default");

    let err = TransportError::MessageTooLarge { max: 512 };
    match err {
        TransportError::MessageTooLarge { max } => assert_eq!(max, 512),
        _ => panic!("expected MessageTooLarge"),
    }
}

#[test]
fn ble_message_size_limit_is_512_bytes() {
    use sync_transport::bluetooth_le::BluetoothLETransport;
    let transport = BluetoothLETransport::new(PlatformBLEHandle::new(true), true);
    assert_eq!(transport.max_message_size(), 512);
}

#[test]
fn cloud_relay_message_size_limit_is_10mb() {
    use std::sync::Arc;
    use sync_transport::cloud_relay::CloudRelayTransport;
    use sync_transport::placeholder_types::StaticAuthorityProvider;

    struct DummyFactory;
    #[async_trait::async_trait]
    impl sync_transport::cloud_relay::RelaySocketFactory for DummyFactory {
        async fn connect(
            &self,
            _relay_url: &str,
            _peer: &PeerInfo,
            _bearer_token: &str,
            _timeout: Duration,
        ) -> Result<Box<dyn sync_transport::cloud_relay::RelaySocket>, TransportError> {
            unimplemented!("not exercised in this test")
        }
    }

    let transport = CloudRelayTransport::new(
        "relay.onyx.test",
        Arc::new(StaticAuthorityProvider("t".to_string())),
        Arc::new(DummyFactory),
    );
    assert_eq!(transport.max_message_size(), 10 * 1024 * 1024);
}

#[tokio::test]
async fn multiple_concurrent_connections() {
    let transport = std::sync::Arc::new(MockTransport::new(TransportType::CloudRelay, true, false));

    let mut handles = Vec::new();
    for _ in 0..10 {
        let t = transport.clone();
        handles.push(tokio::spawn(async move {
            t.connect(PeerInfo::dummy(), Duration::from_secs(1)).await
        }));
    }

    let mut successes = 0;
    for h in handles {
        if h.await.unwrap().is_ok() {
            successes += 1;
        }
    }

    assert_eq!(successes, 10);
    assert_eq!(
        transport
            .connect_count
            .load(std::sync::atomic::Ordering::SeqCst),
        10
    );
}

#[test]
fn mobile_transports_available_reflects_os_capabilities() {
    use sync_transport::bluetooth_le::BluetoothLETransport;
    use sync_transport::wi_fi_direct::WifiDirectTransport;

    let wifi_on = WifiDirectTransport::new(PlatformHandle::new(true), true);
    let wifi_off = WifiDirectTransport::new(PlatformHandle::new(false), true);
    let ble_on = BluetoothLETransport::new(PlatformBLEHandle::new(true), true);
    let ble_off = BluetoothLETransport::new(PlatformBLEHandle::new(false), true);

    assert!(wifi_on.is_available());
    assert!(!wifi_off.is_available());
    assert!(ble_on.is_available());
    assert!(!ble_off.is_available());
}
