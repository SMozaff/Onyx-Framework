//! Session policy per client kind. Single source of truth in Rust; shells and
//! platform code only ask this module what to do.
//!
//! - Desktop, Android, PWA: stay signed in across restarts. The local database
//!   key is kept in the OS secure storage while signed in. An explicit sign-out
//!   removes it, and the next sign-in needs the username and password.
//! - Admin: tighter. Tokens are memory-only (never persisted), a refresh
//!   failure fails closed and requires a fresh login, and no database key is
//!   kept on the device.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientKind {
    Desktop,
    Android,
    Pwa,
    Admin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStorage {
    /// Derived key kept in OS secure storage while signed in.
    OsSecureStorageWhileSignedIn,
    /// Never written to the device.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshFailure {
    /// Keep the session and retry later.
    KeepSessionAndRetry,
    /// Sign out locally and require a fresh login.
    FailClosedRequireLogin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicy {
    pub kind: ClientKind,
    pub tokens_persisted: bool,
    pub stay_signed_in_across_restarts: bool,
    pub key_storage: KeyStorage,
    pub on_refresh_failure: RefreshFailure,
}

impl SessionPolicy {
    pub const fn for_client(kind: ClientKind) -> Self {
        match kind {
            ClientKind::Desktop | ClientKind::Android | ClientKind::Pwa => Self {
                kind,
                tokens_persisted: true,
                stay_signed_in_across_restarts: true,
                key_storage: KeyStorage::OsSecureStorageWhileSignedIn,
                on_refresh_failure: RefreshFailure::KeepSessionAndRetry,
            },
            ClientKind::Admin => Self {
                kind,
                tokens_persisted: false,
                stay_signed_in_across_restarts: false,
                key_storage: KeyStorage::None,
                on_refresh_failure: RefreshFailure::FailClosedRequireLogin,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_clients_stay_signed_in_with_key_in_secure_storage() {
        for kind in [ClientKind::Desktop, ClientKind::Android, ClientKind::Pwa] {
            let p = SessionPolicy::for_client(kind);
            assert!(p.tokens_persisted, "{kind:?}");
            assert!(p.stay_signed_in_across_restarts, "{kind:?}");
            assert_eq!(
                p.key_storage,
                KeyStorage::OsSecureStorageWhileSignedIn,
                "{kind:?}"
            );
            assert_eq!(
                p.on_refresh_failure,
                RefreshFailure::KeepSessionAndRetry,
                "{kind:?}"
            );
        }
    }

    #[test]
    fn admin_is_memory_only_fail_closed_and_keeps_no_key() {
        let p = SessionPolicy::for_client(ClientKind::Admin);
        assert!(!p.tokens_persisted);
        assert!(!p.stay_signed_in_across_restarts);
        assert_eq!(p.key_storage, KeyStorage::None);
        assert_eq!(p.on_refresh_failure, RefreshFailure::FailClosedRequireLogin);
    }
}
