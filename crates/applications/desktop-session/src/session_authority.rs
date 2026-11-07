//! Session-owned relay credential authority for the Tauri desktop shell.
//!
//! This replaces the test-only `StaticAuthorityProvider` in the logged-in
//! composition (P-01 Track A). The rules:
//!
//! - **One owner.** `SessionAuthority` owns the only in-memory copy of the
//!   token pair. `SessionBearerProvider` keeps no copy and reads the current
//!   access token on every `bearer_token` call.
//! - **Fail closed.** With no live session, or after a failed refresh or
//!   revocation, the provider returns `AuthorityError::NoCredentials`. It never
//!   returns an empty or stale token.
//! - **Refresh before expiry.** A read within `REFRESH_SKEW` of expiry rotates
//!   the pair through `POST /api/auth/refresh`. Concurrent readers share one
//!   rotation, because the Worker consumes each refresh token exactly once and
//!   a second use would be treated as replay.
//! - **Atomic rotation.** The new access and refresh tokens are published under
//!   one write lock. The rotated refresh token is persisted before the next
//!   refresh can use it.
//! - **Logout.** Local use is denied first, then the keyring entry is deleted,
//!   and the latest pair is revoked through `POST /api/auth/logout`.
//! - **Redaction.** No token appears in `Debug` output, errors or log fields.
//!
//! Persistence follows `docs/migrations/slint/desktop-token-storage-policy.md`:
//! the keyring holds the refresh token only. Keyring failure is a notice, and
//! the session continues for the current launch.

use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use platform_kernel::{ObjectId, OrganizationId};
use sync_transport::placeholder_types::{AuthorityError, AuthorityProvider};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use crate::secure_storage::{SecureStorage, StorageError};
use crate::session::{self, RefreshError, SessionApi, SessionError, StoredSession};

/// Rotate the access token this long before its stated expiry.
pub const REFRESH_SKEW: Duration = Duration::from_secs(30);

/// Non-secret identity of the live session, for building local state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSummary {
    pub server_address: String,
    pub user_id: ObjectId,
    pub organization_id: OrganizationId,
    pub username: String,
    pub is_admin: bool,
}

/// Result of a successful sign-in.
#[derive(Debug, Clone)]
pub struct SignInOutcome {
    pub summary: SessionSummary,
    /// `false` when the keyring could not store the refresh token. The
    /// session then lasts only for this launch. This is a notice, not an error.
    pub persisted: bool,
}

struct LiveSession {
    /// Identity plus the current refresh token. This is the only in-memory
    /// copy of the refresh token.
    stored: StoredSession,
    access_token: String,
    access_expires_at: Instant,
}

#[derive(Default)]
struct AuthState {
    live: Option<LiveSession>,
    /// Advances when a sign-in starts and when a sign-out completes, so a
    /// logout that was overtaken by a newer sign-in does nothing.
    generation: u64,
    /// While true, no token is handed out. Logout sets this first, then waits
    /// for any in-flight rotation before it revokes the latest pair.
    logging_out: bool,
}

/// The single authoritative session owner for the desktop shell.
pub struct SessionAuthority {
    server_address: String,
    api: Arc<dyn SessionApi>,
    storage: Arc<dyn SecureStorage>,
    state: RwLock<AuthState>,
    /// Serializes login, refresh, restore and logout. Only one rotation runs
    /// at a time.
    refresh_gate: Mutex<()>,
    /// Cancels relay connection attempts tied to the previous session.
    connection_cancellation: Arc<RwLock<CancellationToken>>,
}

impl std::fmt::Debug for SessionAuthority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionAuthority")
            .field("server_address", &self.server_address)
            .field("tokens", &"<redacted>")
            .finish()
    }
}

impl SessionAuthority {
    pub fn new(
        server_address: impl Into<String>,
        api: Arc<dyn SessionApi>,
        storage: Arc<dyn SecureStorage>,
    ) -> Arc<Self> {
        Arc::new(Self {
            server_address: server_address.into().trim_end_matches('/').to_string(),
            api,
            storage,
            state: RwLock::new(AuthState::default()),
            refresh_gate: Mutex::new(()),
            connection_cancellation: Arc::new(RwLock::new(CancellationToken::new())),
        })
    }

    /// Shared cancellation source for the relay socket factory. Logout cancels
    /// it, which also stops connections that have not yet reached the network.
    pub fn connection_cancellation_source(&self) -> Arc<RwLock<CancellationToken>> {
        Arc::clone(&self.connection_cancellation)
    }

    /// Signs in with the server and installs the new pair.
    pub async fn sign_in(
        &self,
        username: &str,
        password: &str,
    ) -> Result<SignInOutcome, SessionError> {
        let _gate = self.refresh_gate.lock().await;

        // Drop any previous session before the network call, so reads fail
        // closed meanwhile. Revoke the old pair so it cannot stay valid.
        let previous = {
            let mut state = self.state.write().await;
            state.generation = state.generation.wrapping_add(1);
            state.logging_out = false;
            state.live.take()
        };
        if let Some(previous) = previous {
            self.revoke_best_effort(&previous).await;
        }
        self.cancel_connection_attempts().await;

        let result = self.api.login(username, password).await?;
        let stored = StoredSession {
            server_address: self.server_address.clone(),
            refresh_token: result.pair.refresh_token.clone(),
            user_id: result.identity.user_id,
            organization_id: result.identity.organization_id,
            username: result.identity.username.clone(),
            is_admin: result.identity.is_admin,
        };
        let summary = summary_of(&stored);
        let persisted = match session::save(&self.storage, &stored).await {
            Ok(()) => true,
            Err(error) => {
                tracing::warn!(
                    error = %error,
                    "keyring unavailable; signed in for this launch only"
                );
                false
            }
        };

        let superseded = {
            let mut state = self.state.write().await;
            state.live = Some(LiveSession {
                stored,
                access_token: result.pair.access_token,
                access_expires_at: Instant::now() + Duration::from_secs(result.pair.expires_in),
            });
            // Logout may have started during the login call. It is waiting for
            // the gate and will revoke this pair, so report the sign-in as
            // superseded rather than successful.
            state.logging_out
        };
        self.cancel_connection_attempts().await;
        if superseded {
            return Err(SessionError::Network(
                "sign-in superseded by sign-out".to_string(),
            ));
        }
        Ok(SignInOutcome { summary, persisted })
    }

    /// Startup restore. Exchanges the stored refresh token for a new access
    /// token and rewrites the keyring record with the rotated refresh token.
    /// Returns `None` when the user must sign in.
    pub async fn restore(&self, stored: StoredSession) -> Option<SessionSummary> {
        let _gate = self.refresh_gate.lock().await;

        // Never reuse credentials issued by another endpoint.
        if stored.server_address != self.server_address {
            tracing::warn!("stored session belongs to another server; sign-in required");
            self.delete_keyring_record().await;
            return None;
        }

        match self.api.refresh(&stored.refresh_token).await {
            Ok(pair) => {
                let rotated = StoredSession {
                    refresh_token: pair.refresh_token.clone(),
                    ..stored
                };
                let summary = summary_of(&rotated);
                if let Err(error) = session::save(&self.storage, &rotated).await {
                    tracing::warn!(error = %error, "could not persist rotated session");
                }
                {
                    let mut state = self.state.write().await;
                    state.generation = state.generation.wrapping_add(1);
                    state.logging_out = false;
                    state.live = Some(LiveSession {
                        stored: rotated,
                        access_token: pair.access_token,
                        access_expires_at: Instant::now() + Duration::from_secs(pair.expires_in),
                    });
                }
                self.cancel_connection_attempts().await;
                Some(summary)
            }
            Err(RefreshError::Rejected) => {
                tracing::warn!("stored session was rejected by the server; sign-in required");
                self.delete_keyring_record().await;
                None
            }
            Err(RefreshError::Unavailable) => {
                // Not a verdict on the session. Keep the stored token for the
                // next launch.
                tracing::warn!("could not restore session (server unavailable); sign-in required");
                None
            }
        }
    }

    /// Identity of the live session, if one exists.
    pub async fn current_summary(&self) -> Option<SessionSummary> {
        let state = self.state.read().await;
        if state.logging_out {
            return None;
        }
        state.live.as_ref().map(|live| summary_of(&live.stored))
    }

    /// Returns a usable access token, rotating it first when it is close to
    /// expiry. `None` means no usable credential, so the user must sign in.
    pub async fn current_access_token(&self) -> Option<String> {
        {
            let state = self.state.read().await;
            if state.logging_out {
                return None;
            }
            let live = state.live.as_ref()?;
            if live.access_expires_at > Instant::now() + REFRESH_SKEW {
                return Some(live.access_token.clone());
            }
        }
        self.rotate_if_needed().await
    }

    async fn rotate_if_needed(&self) -> Option<String> {
        let _gate = self.refresh_gate.lock().await;

        // Another caller may have rotated while this one waited for the gate.
        let presented_refresh = {
            let state = self.state.read().await;
            if state.logging_out {
                return None;
            }
            let live = state.live.as_ref()?;
            if live.access_expires_at > Instant::now() + REFRESH_SKEW {
                return Some(live.access_token.clone());
            }
            live.stored.refresh_token.clone()
        };

        match self.api.refresh(&presented_refresh).await {
            Ok(pair) => {
                let token = pair.access_token.clone();
                let rotated_record = {
                    let mut state = self.state.write().await;
                    let live = state.live.as_mut()?;
                    live.stored.refresh_token = pair.refresh_token;
                    live.access_token = pair.access_token;
                    live.access_expires_at = Instant::now() + Duration::from_secs(pair.expires_in);
                    live.stored.clone()
                };
                if let Err(error) = session::save(&self.storage, &rotated_record).await {
                    // The rotated token is still in memory. If this write fails,
                    // the next launch needs a sign-in, because the stored token
                    // is now consumed.
                    tracing::warn!(error = %error, "could not persist rotated session");
                }
                // Logout may have started during the rotation. Deny local use
                // now. Logout is waiting for the gate and will revoke this pair.
                if self.state.read().await.logging_out {
                    return None;
                }
                Some(token)
            }
            Err(error) => {
                // Fail closed. Both outcomes drop the in-memory pair and stop
                // connection attempts that still hold the old token.
                self.state.write().await.live = None;
                self.cancel_connection_attempts().await;
                if error == RefreshError::Rejected {
                    tracing::warn!("session refresh rejected; sign-in required");
                    self.delete_keyring_record().await;
                } else {
                    tracing::warn!("session refresh unavailable; sign-in required");
                }
                None
            }
        }
    }

    /// Logs out locally first, then revokes the latest pair server-side.
    pub async fn sign_out(&self) {
        let (generation, cancellation) = {
            let mut state = self.state.write().await;
            state.logging_out = true;
            let cancellation = self.connection_cancellation.read().await.clone();
            (state.generation, cancellation)
        };
        // Stop connection attempts that are already in flight.
        cancellation.cancel();

        // Wait for any in-flight rotation. Its new pair must be the one revoked.
        let _gate = self.refresh_gate.lock().await;
        let snapshot = {
            let mut state = self.state.write().await;
            if state.generation != generation || !state.logging_out {
                return;
            }
            let live = state.live.take();
            state.generation = state.generation.wrapping_add(1);
            state.logging_out = false;
            live
        };

        self.delete_keyring_record().await;
        if let Some(live) = snapshot {
            self.revoke_best_effort(&live).await;
        }
    }

    async fn revoke_best_effort(&self, live: &LiveSession) {
        if let Err(error) = self
            .api
            .revoke(&live.access_token, &live.stored.refresh_token)
            .await
        {
            tracing::warn!(error = %error, "server-side session revocation failed");
        }
    }

    async fn delete_keyring_record(&self) {
        match self.storage.delete_secret(session::SESSION_KEY).await {
            Ok(()) | Err(StorageError::NotFound) => {}
            Err(error) => {
                tracing::warn!(error = %error, "could not clear stored session");
            }
        }
    }

    /// Cancels attempts tied to the previous session, then starts a fresh
    /// token for the next one. Used when a new session is installed.
    async fn cancel_connection_attempts(&self) {
        let mut source = self.connection_cancellation.write().await;
        source.cancel();
        *source = CancellationToken::new();
    }
}

fn summary_of(stored: &StoredSession) -> SessionSummary {
    SessionSummary {
        server_address: stored.server_address.clone(),
        user_id: stored.user_id,
        organization_id: stored.organization_id,
        username: stored.username.clone(),
        is_admin: stored.is_admin,
    }
}

/// The relay authority handed to `AppStateConfig`. It holds no credential.
pub struct SessionBearerProvider {
    authority: Arc<SessionAuthority>,
}

impl SessionBearerProvider {
    pub fn new(authority: Arc<SessionAuthority>) -> Self {
        Self { authority }
    }
}

impl std::fmt::Debug for SessionBearerProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionBearerProvider")
            .field("tokens", &"<redacted>")
            .finish()
    }
}

#[async_trait]
impl AuthorityProvider for SessionBearerProvider {
    async fn bearer_token(&self) -> Result<String, AuthorityError> {
        match self.authority.current_access_token().await {
            Some(token) if !token.trim().is_empty() => Ok(token),
            // Logged out, revoked, or refresh failed: sign-in required.
            _ => Err(AuthorityError::NoCredentials),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secure_storage::StorageError;
    use crate::session::{IssuedPair, LoginIdentity, LoginResult};
    use std::collections::{HashMap, VecDeque};
    use std::sync::atomic::{AtomicBool, Ordering};

    const ORIGIN: &str = "https://generated.example.invalid";

    // ---- fakes -----------------------------------------------------------

    #[derive(Default)]
    struct FakeStorage {
        entries: std::sync::Mutex<HashMap<String, Vec<u8>>>,
        fail_store: AtomicBool,
        fail_delete: AtomicBool,
    }

    impl FakeStorage {
        fn raw(&self) -> Option<Vec<u8>> {
            self.entries
                .lock()
                .unwrap()
                .get(session::SESSION_KEY)
                .cloned()
        }
    }

    #[async_trait]
    impl SecureStorage for FakeStorage {
        async fn store_secret(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
            if self.fail_store.load(Ordering::SeqCst) {
                return Err(StorageError::Platform("keyring locked".into()));
            }
            self.entries
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_vec());
            Ok(())
        }

        async fn get_secret(&self, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
            Ok(self.entries.lock().unwrap().get(key).cloned())
        }

        async fn delete_secret(&self, key: &str) -> Result<(), StorageError> {
            if self.fail_delete.load(Ordering::SeqCst) {
                return Err(StorageError::Platform("keyring locked".into()));
            }
            match self.entries.lock().unwrap().remove(key) {
                Some(_) => Ok(()),
                None => Err(StorageError::NotFound),
            }
        }
    }

    #[derive(Default)]
    struct FakeApi {
        login_queue: std::sync::Mutex<VecDeque<Result<LoginResult, SessionError>>>,
        refresh_queue: std::sync::Mutex<VecDeque<Result<IssuedPair, RefreshError>>>,
        refresh_delay: std::sync::Mutex<Duration>,
        refresh_calls: std::sync::Mutex<Vec<String>>,
        revoke_calls: std::sync::Mutex<Vec<(String, String)>>,
    }

    #[async_trait]
    impl SessionApi for FakeApi {
        async fn login(&self, _u: &str, _p: &str) -> Result<LoginResult, SessionError> {
            self.login_queue
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(SessionError::InvalidCredentials))
        }

        async fn refresh(&self, refresh_token: &str) -> Result<IssuedPair, RefreshError> {
            self.refresh_calls
                .lock()
                .unwrap()
                .push(refresh_token.to_string());
            let delay = *self.refresh_delay.lock().unwrap();
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            self.refresh_queue
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or(Err(RefreshError::Unavailable))
        }

        async fn revoke(&self, access: &str, refresh: &str) -> Result<(), SessionError> {
            self.revoke_calls
                .lock()
                .unwrap()
                .push((access.to_string(), refresh.to_string()));
            Ok(())
        }
    }

    fn login_ok(expires_in: u64, n: usize) -> Result<LoginResult, SessionError> {
        Ok(LoginResult {
            pair: pair(n, expires_in),
            identity: LoginIdentity {
                user_id: ObjectId::new_random(),
                organization_id: OrganizationId::new_random(),
                username: "generated-user".into(),
                is_admin: false,
            },
        })
    }

    fn pair(n: usize, expires_in: u64) -> IssuedPair {
        IssuedPair {
            access_token: format!("generated-access-{n}"),
            refresh_token: format!("generated-refresh-{n}"),
            expires_in,
        }
    }

    fn fixture() -> (Arc<SessionAuthority>, Arc<FakeApi>, Arc<FakeStorage>) {
        let api = Arc::new(FakeApi::default());
        let storage = Arc::new(FakeStorage::default());
        let authority = SessionAuthority::new(ORIGIN, api.clone(), storage.clone());
        (authority, api, storage)
    }

    fn provider(authority: &Arc<SessionAuthority>) -> SessionBearerProvider {
        SessionBearerProvider::new(Arc::clone(authority))
    }

    fn push_login(api: &FakeApi, result: Result<LoginResult, SessionError>) {
        api.login_queue.lock().unwrap().push_back(result);
    }

    fn push_refresh(api: &FakeApi, result: Result<IssuedPair, RefreshError>) {
        api.refresh_queue.lock().unwrap().push_back(result);
    }

    // ---- provider contract ----------------------------------------------

    #[tokio::test]
    async fn logged_out_provider_fails_closed_with_no_credentials() {
        let (authority, api, _storage) = fixture();
        let result = provider(&authority).bearer_token().await;
        assert!(matches!(result, Err(AuthorityError::NoCredentials)));
        assert!(api.refresh_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn valid_access_token_is_returned_from_the_live_session() {
        let (authority, api, _storage) = fixture();
        push_login(&api, login_ok(3600, 0));
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();

        let token = provider(&authority).bearer_token().await.unwrap();
        assert_eq!(token, "generated-access-0");
        assert!(api.refresh_calls.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn token_replacement_after_refresh_publishes_the_rotated_pair() {
        let (authority, api, storage) = fixture();
        // expires_in below REFRESH_SKEW, so the first read rotates.
        push_login(&api, login_ok(1, 0));
        push_refresh(&api, Ok(pair(1, 3600)));
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();

        let token = provider(&authority).bearer_token().await.unwrap();
        assert_eq!(token, "generated-access-1");
        assert_eq!(
            api.refresh_calls.lock().unwrap().as_slice(),
            ["generated-refresh-0"]
        );
        // The rotated refresh token replaced the old one in the keyring.
        let stored = session::load(&(storage.clone() as Arc<dyn SecureStorage>))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.refresh_token, "generated-refresh-1");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_reads_during_replacement_never_panic_or_return_empty() {
        let (authority, api, _storage) = fixture();
        push_login(&api, login_ok(1, 0));
        push_refresh(&api, Ok(pair(1, 3600)));
        *api.refresh_delay.lock().unwrap() = Duration::from_millis(50);
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();

        let mut readers = Vec::new();
        for _ in 0..32 {
            let p = provider(&authority);
            readers.push(tokio::spawn(async move { p.bearer_token().await }));
        }
        for reader in readers {
            let token = reader.await.unwrap().expect("reader must get a token");
            assert_eq!(token, "generated-access-1");
        }
        // Single-flight: one rotation serves all readers. A second use of the
        // same refresh token would be treated as replay by the Worker.
        assert_eq!(api.refresh_calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn refresh_unavailable_clears_session_and_keeps_keyring_record() {
        let (authority, api, storage) = fixture();
        push_login(&api, login_ok(1, 0));
        // Empty refresh queue: the fake reports Unavailable.
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();

        let p = provider(&authority);
        assert!(matches!(
            p.bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
        assert!(matches!(
            p.bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
        assert_eq!(api.refresh_calls.lock().unwrap().len(), 1);
        assert!(
            storage.raw().is_some(),
            "transient failure keeps the stored token"
        );
    }

    #[tokio::test]
    async fn revoked_refresh_token_clears_session_and_keyring() {
        let (authority, api, storage) = fixture();
        push_login(&api, login_ok(1, 0));
        push_refresh(&api, Err(RefreshError::Rejected));
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();

        assert!(matches!(
            provider(&authority).bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
        assert!(
            storage.raw().is_none(),
            "revoked session must leave no stored token"
        );
        assert!(authority.current_summary().await.is_none());
    }

    #[tokio::test]
    async fn logout_clears_memory_and_keyring_and_revokes_the_latest_pair() {
        let (authority, api, storage) = fixture();
        push_login(&api, login_ok(1, 0));
        push_refresh(&api, Ok(pair(1, 3600)));
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();
        provider(&authority).bearer_token().await.unwrap(); // rotates to pair 1

        authority.sign_out().await;

        assert!(matches!(
            provider(&authority).bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
        assert!(storage.raw().is_none());
        assert_eq!(
            api.revoke_calls.lock().unwrap().as_slice(),
            [(
                "generated-access-1".to_string(),
                "generated-refresh-1".to_string()
            )]
        );
    }

    #[tokio::test]
    async fn logout_during_in_flight_rotation_revokes_the_rotated_pair() {
        let (authority, api, _storage) = fixture();
        push_login(&api, login_ok(1, 0));
        push_refresh(&api, Ok(pair(1, 3600)));
        *api.refresh_delay.lock().unwrap() = Duration::from_millis(150);
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();

        let reader = {
            let p = provider(&authority);
            tokio::spawn(async move { p.bearer_token().await })
        };
        tokio::time::sleep(Duration::from_millis(30)).await;
        authority.sign_out().await;
        let _ = reader.await.unwrap();

        assert_eq!(
            api.revoke_calls.lock().unwrap().as_slice(),
            [(
                "generated-access-1".to_string(),
                "generated-refresh-1".to_string()
            )],
            "logout must revoke the rotated pair, not the consumed one"
        );
        assert!(matches!(
            provider(&authority).bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
    }

    #[tokio::test]
    async fn keyring_unavailable_on_sign_in_is_a_launch_only_notice() {
        let (authority, api, storage) = fixture();
        storage.fail_store.store(true, Ordering::SeqCst);
        push_login(&api, login_ok(3600, 0));

        let outcome = authority
            .sign_in("generated-user", "generated-password")
            .await
            .expect("keyring failure must not fail sign-in");
        assert!(!outcome.persisted, "notice: not persisted");
        assert_eq!(
            provider(&authority).bearer_token().await.unwrap(),
            "generated-access-0",
            "signed in for this launch"
        );
        assert!(storage.raw().is_none());
    }

    #[tokio::test]
    async fn restore_exchanges_the_stored_refresh_token_and_rewrites_the_record() {
        let (authority, api, storage) = fixture();
        let stored = StoredSession {
            server_address: ORIGIN.to_string(),
            refresh_token: "generated-refresh-old".into(),
            user_id: ObjectId::new_random(),
            organization_id: OrganizationId::new_random(),
            username: "generated-user".into(),
            is_admin: true,
        };
        push_refresh(&api, Ok(pair(7, 3600)));

        let summary = authority.restore(stored).await.expect("restore succeeds");
        assert!(summary.is_admin);
        assert_eq!(
            api.refresh_calls.lock().unwrap().as_slice(),
            ["generated-refresh-old"]
        );
        assert_eq!(
            provider(&authority).bearer_token().await.unwrap(),
            "generated-access-7"
        );
        let saved = session::load(&(storage.clone() as Arc<dyn SecureStorage>))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.refresh_token, "generated-refresh-7");
    }

    #[tokio::test]
    async fn restore_rejected_clears_the_keyring_record() {
        let (authority, api, storage) = fixture();
        let stored = StoredSession {
            server_address: ORIGIN.to_string(),
            refresh_token: "generated-refresh-old".into(),
            user_id: ObjectId::new_random(),
            organization_id: OrganizationId::new_random(),
            username: "generated-user".into(),
            is_admin: false,
        };
        session::save(&(storage.clone() as Arc<dyn SecureStorage>), &stored)
            .await
            .unwrap();
        push_refresh(&api, Err(RefreshError::Rejected));
        let loaded = session::load(&(storage.clone() as Arc<dyn SecureStorage>))
            .await
            .unwrap()
            .unwrap();

        assert!(authority.restore(loaded).await.is_none());
        assert!(storage.raw().is_none());
    }

    #[tokio::test]
    async fn restore_unavailable_keeps_the_keyring_record() {
        let (authority, api, storage) = fixture();
        let stored = StoredSession {
            server_address: ORIGIN.to_string(),
            refresh_token: "generated-refresh-old".into(),
            user_id: ObjectId::new_random(),
            organization_id: OrganizationId::new_random(),
            username: "generated-user".into(),
            is_admin: false,
        };
        session::save(&(storage.clone() as Arc<dyn SecureStorage>), &stored)
            .await
            .unwrap();
        push_refresh(&api, Err(RefreshError::Unavailable));

        assert!(authority.restore(stored).await.is_none());
        assert!(storage.raw().is_some());
        assert!(matches!(
            provider(&authority).bearer_token().await,
            Err(AuthorityError::NoCredentials)
        ));
    }

    #[tokio::test]
    async fn restore_never_reuses_a_record_from_another_server() {
        let (authority, api, storage) = fixture();
        let stored = StoredSession {
            server_address: "https://retired.example.invalid".into(),
            refresh_token: "generated-refresh-old".into(),
            user_id: ObjectId::new_random(),
            organization_id: OrganizationId::new_random(),
            username: "generated-user".into(),
            is_admin: false,
        };
        session::save(&(storage.clone() as Arc<dyn SecureStorage>), &stored)
            .await
            .unwrap();

        assert!(authority.restore(stored).await.is_none());
        assert!(api.refresh_calls.lock().unwrap().is_empty());
        assert!(storage.raw().is_none());
    }

    #[tokio::test]
    async fn signing_in_again_revokes_the_previous_pair() {
        let (authority, api, _storage) = fixture();
        push_login(&api, login_ok(3600, 0));
        push_login(&api, login_ok(3600, 5));
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();

        assert_eq!(
            api.revoke_calls.lock().unwrap().as_slice(),
            [(
                "generated-access-0".to_string(),
                "generated-refresh-0".to_string()
            )]
        );
        assert_eq!(
            provider(&authority).bearer_token().await.unwrap(),
            "generated-access-5"
        );
    }

    #[tokio::test]
    async fn persisted_record_and_provider_never_expose_the_access_token() {
        let (authority, api, storage) = fixture();
        push_login(&api, login_ok(1, 0));
        push_refresh(&api, Ok(pair(1, 3600)));
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();
        provider(&authority).bearer_token().await.unwrap();

        let raw = String::from_utf8(storage.raw().unwrap()).unwrap();
        assert!(
            !raw.contains("generated-access-"),
            "access token must stay in memory"
        );

        let debug = format!("{authority:?} {:?}", provider(&authority));
        assert!(!debug.contains("generated-access-"));
        assert!(!debug.contains("generated-refresh-"));

        let error = AuthorityError::NoCredentials.to_string();
        assert!(!error.contains("generated-"));
    }

    #[tokio::test]
    async fn logged_out_authority_reports_no_summary() {
        let (authority, _api, _storage) = fixture();
        assert!(authority.current_summary().await.is_none());
        assert!(authority.current_access_token().await.is_none());
    }

    #[tokio::test]
    async fn logout_cancels_connection_attempts_tied_to_the_session() {
        let (authority, api, _storage) = fixture();
        push_login(&api, login_ok(3600, 0));
        authority
            .sign_in("generated-user", "generated-password")
            .await
            .unwrap();
        let before = authority
            .connection_cancellation_source()
            .read()
            .await
            .clone();

        authority.sign_out().await;

        assert!(before.is_cancelled());
    }
}
