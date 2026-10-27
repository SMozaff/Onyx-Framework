//! Native Slint composition root.
//!
//! Keeps authentication and relay authority in Rust. The UI receives only
//! non-secret identity fields; access and refresh tokens remain in the
//! in-memory session service and are never exposed as Slint properties.

use std::sync::Arc;

use platform_kernel::ReplicaId;
use sync_transport::{
    cloud_relay::CloudRelayTransport,
    placeholder_types::AuthorityProvider,
};

use crate::{
    authority::{SessionAuthorityProvider, SessionTokenSource},
    session_service::{SessionIdentity, SessionService},
};
use relay_websocket_adapter::TungsteniteRelaySocketFactory;

/// The same canonical API origin used by the current desktop-shell login flow.
pub const ONYX_API_ORIGIN: &str = "https://onyx-framework.soheil-mozaffari.workers.dev";

pub struct SlintComposition {
    pub session: Arc<SessionService>,
    pub relay: Arc<CloudRelayTransport>,
}

impl SlintComposition {
    pub fn new() -> Result<Arc<Self>, crate::authority::SessionSourceError> {
        let session = SessionService::new(ONYX_API_ORIGIN)?;
        let token_source: Arc<dyn SessionTokenSource> = session.clone();
        let authority: Arc<dyn AuthorityProvider> =
            Arc::new(SessionAuthorityProvider::new(token_source));
        let socket_factory = Arc::new(TungsteniteRelaySocketFactory::new(
            ReplicaId::new_random(),
        ));
        let relay = Arc::new(CloudRelayTransport::new(
            ONYX_API_ORIGIN,
            authority,
            socket_factory,
        ));

        Ok(Arc::new(Self { session, relay }))
    }

    pub async fn login(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SessionIdentity, crate::authority::SessionSourceError> {
        self.session.login(username, password).await
    }

    pub async fn logout(&self) {
        self.session.logout().await;
    }
}
