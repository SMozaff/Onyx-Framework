//! Bluetooth LE transport. Frozen contract per Team Prompt 4 §4.4.
//! Mobile-to-mobile fallback when Wi-Fi Direct is unavailable.
//! Radio stack itself is out of scope (Team 5 integrates via
//! `sync-transport-mobile` FFI); this is the trait-conforming shell plus
//! the `encryption_enabled` flag per DECISIONS.md §6.

use crate::discovery::PeerInfo;
use crate::message::SyncMessage;
use crate::placeholder_types::{PlatformBLEHandle, PlatformHandleError};
use crate::transport_trait::{Connection, Listener, Transport, TransportError, TransportType};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::time::Duration;

/// Bluetooth LE transport.
/// Uses GATT-based data channel with chunking for larger messages
/// (chunking itself lives in the platform bridge, since BLE's MTU limit
/// means "one `SyncMessage`" and "one BLE packet" are different things —
/// see `max_message_size` below).
pub struct BluetoothLETransport {
    platform_handle: PlatformBLEHandle,
    /// Whether platform BLE encryption is enabled for this link. Set by
    /// Team 5 per DECISIONS.md §6.
    encryption_enabled: bool,
}

impl BluetoothLETransport {
    pub fn new(platform_handle: PlatformBLEHandle, encryption_enabled: bool) -> Self {
        Self {
            platform_handle,
            encryption_enabled,
        }
    }
}

#[async_trait]
impl Transport for BluetoothLETransport {
    fn transport_type(&self) -> TransportType {
        TransportType::BluetoothLE
    }

    fn is_available(&self) -> bool {
        self.platform_handle.is_ble_available() && self.encryption_enabled
    }

    async fn connect(
        &self,
        peer: PeerInfo,
        timeout: Duration,
    ) -> Result<Box<dyn Connection>, TransportError> {
        if !self.encryption_enabled {
            return Err(TransportError::Platform(
                "BLE link is not encrypted".to_string(),
            ));
        }
        let _connection = self
            .platform_handle
            .connect_to_peer(&peer, timeout)
            .await
            .map_err(map_platform_err)?;
        Ok(Box::new(BLEConnection))
    }

    async fn listen(
        &self,
        _listen_addr: Option<SocketAddr>,
    ) -> Result<Box<dyn Listener>, TransportError> {
        let _listener = self
            .platform_handle
            .start_advertising()
            .await
            .map_err(map_platform_err)?;
        Ok(Box::new(BLEListener))
    }

    fn max_message_size(&self) -> usize {
        // BLE MTU is typically limited to ~512 bytes per packet.
        // Larger messages must be chunked by the platform bridge layer
        // (sync-transport-mobile), which is out of this crate's scope.
        512
    }

    fn provides_encryption(&self) -> bool {
        self.encryption_enabled
    }
}

fn map_platform_err(e: PlatformHandleError) -> TransportError {
    TransportError::Platform(e.0)
}

struct BLEConnection;

#[async_trait]
impl Connection for BLEConnection {
    async fn request_response(
        &mut self,
        _request: SyncMessage,
        _timeout: Duration,
    ) -> Result<SyncMessage, TransportError> {
        Err(TransportError::ConnectionLost)
    }

    async fn send(&mut self, _message: SyncMessage) -> Result<(), TransportError> {
        Err(TransportError::ConnectionLost)
    }

    async fn recv(&mut self) -> Result<SyncMessage, TransportError> {
        Err(TransportError::ConnectionLost)
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::BluetoothLE
    }
}

struct BLEListener;

#[async_trait]
impl Listener for BLEListener {
    async fn accept(&mut self) -> Result<Box<dyn Connection>, TransportError> {
        Err(TransportError::ConnectionLost)
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_size_limit_matches_ble_mtu() {
        let transport = BluetoothLETransport::new(PlatformBLEHandle::new(true), true);
        assert_eq!(transport.max_message_size(), 512);
    }

    #[test]
    fn unavailable_when_encryption_disabled() {
        let transport = BluetoothLETransport::new(PlatformBLEHandle::new(true), false);
        assert!(!transport.is_available());
    }

    #[tokio::test]
    async fn connect_rejects_when_not_encrypted() {
        let transport = BluetoothLETransport::new(PlatformBLEHandle::new(true), false);
        let result = transport
            .connect(PeerInfo::dummy(), Duration::from_secs(1))
            .await;
        // `Box<dyn Connection>` (the Ok side) isn't `Debug` (the trait
        // doesn't require it), so `unwrap_err()` doesn't compile here —
        // caught by `cargo check`, not assumed. Matching explicitly instead.
        match result {
            Err(TransportError::Platform(_)) => {}
            _ => panic!("expected TransportError::Platform, got a different result"),
        }
    }
}
