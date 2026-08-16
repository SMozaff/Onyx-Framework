//! Wi-Fi Direct transport. Frozen contract per Team Prompt 4 §4.3.
//! Desktop: uses local network sockets + mDNS (not implemented here — out
//! of scope per the mission statement: "You do not build the full Wi-Fi
//! Direct/BLE radio stacks on mobile — those are platform-specific
//! implementations that Team 5 integrates").
//! Mobile: uses platform APIs (Multipeer Connectivity / WifiP2pManager) via
//! `sync-transport-mobile` FFI, called by Team 5's native client code.
//!
//! `encryption_enabled` per DECISIONS.md §6: Team 5 sets this based on
//! platform capability (WPA2 on the active Wi-Fi Direct group). This crate
//! only carries the flag through; it does not perform crypto negotiation
//! for this transport (that's `platform_handle`'s / Team 5's job).

use crate::discovery::PeerInfo;
use crate::placeholder_types::{PlatformHandle, PlatformHandleError, PlatformStream};
use crate::transport_trait::{Connection, Listener, Transport, TransportError, TransportType};
use async_trait::async_trait;
use std::net::SocketAddr;
use std::time::Duration;

/// Wi-Fi Direct transport.
pub struct WifiDirectTransport {
    platform_handle: PlatformHandle,
    /// Whether the active Wi-Fi Direct link is encrypted (WPA2). Set by
    /// Team 5 based on platform capability at construction time. Per
    /// DECISIONS.md §6 and Team Prompt 4 §7.1 ("No approved transport
    /// carries plaintext"): `provides_encryption()` returns this flag, and
    /// callers (the selector) should treat `false` as "not an approved
    /// transport right now" rather than silently sending plaintext.
    encryption_enabled: bool,
}

impl WifiDirectTransport {
    pub fn new(platform_handle: PlatformHandle, encryption_enabled: bool) -> Self {
        Self {
            platform_handle,
            encryption_enabled,
        }
    }
}

#[async_trait]
impl Transport for WifiDirectTransport {
    fn transport_type(&self) -> TransportType {
        TransportType::WifiDirect
    }

    fn is_available(&self) -> bool {
        // Per §7.1, Wi-Fi Direct without encryption is not an approved
        // transport, so it must not be reported as available. See
        // DECISIONS.md §6.
        self.platform_handle.is_wifi_direct_available() && self.encryption_enabled
    }

    async fn connect(
        &self,
        peer: PeerInfo,
        timeout: Duration,
    ) -> Result<Box<dyn Connection>, TransportError> {
        if !self.encryption_enabled {
            return Err(TransportError::Platform(
                "Wi-Fi Direct link is not encrypted".to_string(),
            ));
        }
        let stream = self
            .platform_handle
            .connect_to_peer(&peer, timeout)
            .await
            .map_err(map_platform_err)?;
        Ok(Box::new(WifiDirectConnection { _stream: stream }))
    }

    async fn listen(
        &self,
        _listen_addr: Option<SocketAddr>,
    ) -> Result<Box<dyn Listener>, TransportError> {
        let listener = self
            .platform_handle
            .start_advertising()
            .await
            .map_err(map_platform_err)?;
        Ok(Box::new(WifiDirectListener { _handle: listener }))
    }

    fn max_message_size(&self) -> usize {
        1024 * 1024 // 1 MB
    }

    fn provides_encryption(&self) -> bool {
        self.encryption_enabled
    }
}

fn map_platform_err(e: PlatformHandleError) -> TransportError {
    TransportError::Platform(e.0)
}

struct WifiDirectConnection {
    _stream: PlatformStream,
}

#[async_trait]
impl Connection for WifiDirectConnection {
    async fn request_response(
        &mut self,
        _request: crate::message::SyncMessage,
        _timeout: Duration,
    ) -> Result<crate::message::SyncMessage, TransportError> {
        // Byte-level I/O over the platform stream is provided by Team 5's
        // FFI bridge (sync-transport-mobile); this stub has no real socket
        // to read/write, so it reports the connection as lost rather than
        // fabricating a response. See mission statement: Team 4 does not
        // build the mobile radio stack itself.
        Err(TransportError::ConnectionLost)
    }

    async fn send(&mut self, _message: crate::message::SyncMessage) -> Result<(), TransportError> {
        Err(TransportError::ConnectionLost)
    }

    async fn recv(&mut self) -> Result<crate::message::SyncMessage, TransportError> {
        Err(TransportError::ConnectionLost)
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        Ok(())
    }

    fn transport_type(&self) -> TransportType {
        TransportType::WifiDirect
    }
}

struct WifiDirectListener {
    _handle: crate::placeholder_types::PlatformListenerHandle,
}

#[async_trait]
impl Listener for WifiDirectListener {
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
    fn unavailable_when_platform_reports_unavailable() {
        let transport = WifiDirectTransport::new(PlatformHandle::new(false), true);
        assert!(!transport.is_available());
    }

    #[test]
    fn unavailable_when_encryption_disabled_even_if_platform_ready() {
        let transport = WifiDirectTransport::new(PlatformHandle::new(true), false);
        assert!(
            !transport.is_available(),
            "an unencrypted Wi-Fi Direct link must never be reported available"
        );
    }

    #[test]
    fn available_when_platform_ready_and_encrypted() {
        let transport = WifiDirectTransport::new(PlatformHandle::new(true), true);
        assert!(transport.is_available());
    }

    #[tokio::test]
    async fn mobile_transports_available_reflects_os_capabilities() {
        let available = WifiDirectTransport::new(PlatformHandle::new(true), true);
        let unavailable = WifiDirectTransport::new(PlatformHandle::new(false), true);
        assert!(available.is_available());
        assert!(!unavailable.is_available());
    }
}
