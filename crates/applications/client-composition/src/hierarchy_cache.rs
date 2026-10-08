//! A local, cached slice of the organization's reporting-line tree, used
//! to answer one question offline: *"is the current user this task/
//! mission owner's direct manager?"*
//!
//! # Why this exists
//! `desktop-shell`'s and `mobile-core`'s embedded `AppState` each compose
//! only the domain aggregates they need offline
//! (`client_composition::AppState`'s own doc comment) — the org's
//! account/reporting-line directory (`security_application::UserStore`)
//! is not one of them, and never has been. That was fine as long as
//! nothing running locally needed to answer an authority question. It
//! stopped being fine once a real, confirmed gap was found:
//! `TaskDecisionHandler`/`MissionDecisionHandler` execute `ApproveTask`/
//! `RejectTask`/`RejectApproval`/`ActivateMission` with no check at all
//! on *who* is issuing the command — any authenticated user could
//! approve any task, regardless of any real relationship to it.
//! Todo/Target-list verification and Staff Loans already have real,
//! working authority resolution (`api_server::verifier_resolution`) —
//! Task/Mission approval never had an equivalent.
//!
//! # Why this lives in `client-composition`, not `desktop-shell`
//! Originally built inside `desktop-shell` (a binary crate) when it was
//! the only client needing this. `mobile-core` needed the identical
//! lookup logic for the same reason, but a binary crate cannot be a
//! library dependency of another crate — the type had to move somewhere
//! both binaries actually share. `client-composition` is that place: it
//! already houses `AppState`/`DenyAllOwnerAuthority` and is already a
//! real dependency of both `desktop-shell` and `mobile-core`, so this is
//! not a new dependency edge, just relocating a type to where it always
//! should have lived once a second consumer existed.
//!
//! # Why the HTTP fetch and the cache-replacement are split apart
//! `desktop-shell` has no other HTTP transport at all, so [`refresh`]
//! (fetch-then-replace) is the right shape for it. `mobile-core` is
//! different: its FFI boundary is used from a Dart app that *already*
//! has an authenticated HTTP client (`OnyxHttpApi`/`OnyxHttpAuthApi`,
//! `mobile/lib/net/`) — re-fetching independently from Rust would
//! duplicate a real, working HTTP/auth stack and create a second place
//! for the session token to drift out of sync. So the actual network
//! fetch happens in Dart; Rust only needs the "replace cache from
//! already-fetched, already-parsed data" half. [`replace_from_wire`] is
//! that shared half — [`refresh`] is a thin fetch-then-call-it wrapper
//! for `desktop-shell`'s use; `mobile-core`'s new `mobile_core_set_
//! hierarchy` FFI function parses the JSON Dart already fetched and
//! calls [`replace_from_wire`] directly. One real implementation of the
//! parsing/lookup logic, not two.
//!
//! [`refresh`]: HierarchyCache::refresh
//! [`replace_from_wire`]: HierarchyCache::replace_from_wire
//!
//! # Why a local cache, not a live server round-trip per approval
//! This app's offline-first design is why `desktop-shell`/`mobile-core`
//! exist as distinct architectures from `admin-shell` in the first
//! place, and requiring a network call at the exact moment of every
//! approval would be inconsistent with that. Fetching the tree once (at
//! login, and on explicit refresh) and checking it locally afterward
//! matches how the rest of these apps already work.
//!
//! # Scope
//! Only direct-manager (tree-parent) authority — not the loan/
//! escalation widening `verifier_resolution` also does for lists. If
//! Task/Mission approval later needs that same widening, that should be
//! a new, explicit decision, not something added silently here.

use std::collections::HashMap;
use std::sync::Arc;

use platform_kernel::ObjectId;
use serde::Deserialize;
use tokio::sync::RwLock;

/// One entry from `GET /api/users/hierarchy` — mirrors
/// `api_server::routes::admin::HierarchyUserDto` field-for-field. `pub`
/// so `mobile-core` can deserialize the exact same shape from the JSON
/// Dart hands it over FFI, without a second, hand-maintained copy of
/// this shape drifting out of sync with the one `desktop-shell`'s
/// `refresh` and the server's own DTO already agree on.
#[derive(Debug, Clone, Deserialize)]
pub struct HierarchyUserWire {
    pub id: String,
    pub parent_user_id: Option<String>,
    pub is_admin: bool,
}

/// One org member's authority-relevant facts, keyed by their id in the
/// cache below. Parsed once out of `HierarchyUserWire`'s raw strings so
/// every lookup afterward is a cheap map access, not a re-parse.
#[derive(Debug, Clone)]
struct HierarchyEntry {
    parent_user_id: Option<ObjectId>,
    is_admin: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum HierarchyError {
    #[error("network error contacting server: {0}")]
    Network(String),
    #[error("server rejected the request: HTTP {0}")]
    UnexpectedStatus(u16),
    #[error("server returned an unexpected response: {0}")]
    UnexpectedResponse(String),
    #[error("id in server response is not a valid UUID: {0}")]
    InvalidId(String),
}

/// Holds the most recently fetched hierarchy slice. Cheap to clone
/// (`Arc`-backed); intended to live once in `AppState`-adjacent shared
/// state. Deliberately interior-mutable (`Arc<RwLock<...>>`) rather than
/// requiring a rebuild of whatever holds it: `mobile-core` constructs
/// its `AppState` (and the `Arc<dyn OwnerAuthority>` handed into it)
/// before login/hierarchy-fetch can possibly have happened, and this
/// shape lets the cache be populated later, in place — every subsequent
/// authority check automatically sees the update, with no `AppState`
/// rebuild needed.
#[derive(Clone, Default)]
pub struct HierarchyCache {
    inner: Arc<RwLock<HashMap<ObjectId, HierarchyEntry>>>,
}

impl HierarchyCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fetches `GET {server_address}/api/users/hierarchy` and replaces
    /// the cache's contents via [`replace_from_wire`](Self::replace_from_wire).
    /// Called at login (see `desktop-shell::lib.rs`'s `login` command)
    /// and available for an explicit manual refresh if the UI ever wants
    /// one; not auto-refreshed on a timer, since staleness here only
    /// matters at the moment of an approval action, and login already
    /// guarantees freshness for that session.
    ///
    /// `mobile-core` does not call this — see this module's doc comment
    /// for why its fetch happens in Dart instead.
    pub async fn refresh(
        &self,
        server_address: &str,
        access_token: &str,
    ) -> Result<(), HierarchyError> {
        let url = format!(
            "{}/api/users/hierarchy",
            server_address.trim_end_matches('/')
        );
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|e| HierarchyError::Network(e.to_string()))?;

        let response = client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|e| HierarchyError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(HierarchyError::UnexpectedStatus(response.status().as_u16()));
        }

        let wire: Vec<HierarchyUserWire> = response
            .json()
            .await
            .map_err(|e| HierarchyError::UnexpectedResponse(e.to_string()))?;

        self.replace_from_wire(wire).await
    }

    /// Parses `json` as the same `Vec<HierarchyUserWire>` shape
    /// `GET /api/users/hierarchy` returns, and replaces the cache's
    /// contents from it. This is what `mobile-core`'s
    /// `mobile_core_set_hierarchy` FFI function calls with the JSON
    /// Dart's `OnyxHttpApi` already fetched — the mobile-side equivalent
    /// of [`refresh`](Self::refresh) minus the HTTP request itself.
    pub async fn load_from_json(&self, json: &str) -> Result<(), HierarchyError> {
        let wire: Vec<HierarchyUserWire> = serde_json::from_str(json)
            .map_err(|e| HierarchyError::UnexpectedResponse(e.to_string()))?;
        self.replace_from_wire(wire).await
    }

    /// The shared "replace cache from parsed wire data" logic both
    /// [`refresh`](Self::refresh) and [`load_from_json`](Self::load_from_json)
    /// call — one real implementation of id-parsing and map-building, not
    /// two. Replaces wholesale, not merged: a member who was deactivated
    /// or reassigned since the last fetch must actually disappear/
    /// update, not linger from a stale entry.
    pub async fn replace_from_wire(
        &self,
        wire: Vec<HierarchyUserWire>,
    ) -> Result<(), HierarchyError> {
        let mut parsed = HashMap::with_capacity(wire.len());
        for entry in wire {
            let id = ObjectId::from_uuid_str(&entry.id)
                .map_err(|_| HierarchyError::InvalidId(entry.id.clone()))?;
            let parent_user_id = match entry.parent_user_id {
                Some(raw) => Some(
                    ObjectId::from_uuid_str(&raw).map_err(|_| HierarchyError::InvalidId(raw))?,
                ),
                None => None,
            };
            parsed.insert(
                id,
                HierarchyEntry {
                    parent_user_id,
                    is_admin: entry.is_admin,
                },
            );
        }

        *self.inner.write().await = parsed;
        Ok(())
    }

    /// Empties the cache — see `desktop-shell::lib.rs`'s `logout` call
    /// site for why this is needed defensively even though `login`
    /// always overwrites the cache's contents on its own.
    pub async fn clear(&self) {
        self.inner.write().await.clear();
    }

    /// Whether `actor_id` is authorized to approve/reject a Task or
    /// Mission owned by `owner_id`: `actor_id` is an Admin, or
    /// `actor_id` is `owner_id`'s direct manager (`parent_user_id`) per
    /// the cached tree. Matches `verifier_resolution::is_authorized_verifier`'s
    /// tree-parent case exactly, minus the loan/escalation widening —
    /// see this module's doc comment for why that's excluded here.
    ///
    /// Returns `false` (not an error) if either id is missing from the
    /// cache — an owner or actor the cache doesn't know about has no
    /// resolvable authority relationship, the same "no verifier found
    /// is a valid state" reasoning `verifier_resolution` itself
    /// documents.
    pub async fn is_authorized_to_decide(&self, actor_id: ObjectId, owner_id: ObjectId) -> bool {
        let cache = self.inner.read().await;
        let Some(actor) = cache.get(&actor_id) else {
            return false;
        };
        if actor.is_admin {
            return true;
        }
        let Some(owner) = cache.get(&owner_id) else {
            return false;
        };
        owner.parent_user_id == Some(actor_id)
    }
}

#[async_trait::async_trait]
impl api_server::OwnerAuthority for HierarchyCache {
    async fn is_authorized(&self, actor: ObjectId, owner: ObjectId) -> bool {
        self.is_authorized_to_decide(actor, owner).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(byte: u8) -> ObjectId {
        ObjectId([byte; 16])
    }

    async fn cache_with(entries: Vec<(ObjectId, Option<ObjectId>, bool)>) -> HierarchyCache {
        let cache = HierarchyCache::new();
        let map: HashMap<ObjectId, HierarchyEntry> = entries
            .into_iter()
            .map(|(uid, parent, is_admin)| {
                (
                    uid,
                    HierarchyEntry {
                        parent_user_id: parent,
                        is_admin,
                    },
                )
            })
            .collect();
        // Populate directly rather than via `refresh`/`load_from_json` —
        // these tests exercise `is_authorized_to_decide`'s logic, not
        // either data-entry path.
        *cache.inner.write().await = map;
        cache
    }

    #[tokio::test]
    async fn direct_manager_is_authorized() {
        let owner = id(1);
        let manager = id(2);
        let cache = cache_with(vec![(owner, Some(manager), false), (manager, None, false)]).await;
        assert!(cache.is_authorized_to_decide(manager, owner).await);
    }

    #[tokio::test]
    async fn unrelated_user_is_not_authorized() {
        let owner = id(1);
        let manager = id(2);
        let stranger = id(3);
        let cache = cache_with(vec![(owner, Some(manager), false), (manager, None, false)]).await;
        assert!(!cache.is_authorized_to_decide(stranger, owner).await);
    }

    #[tokio::test]
    async fn admin_is_always_authorized_regardless_of_tree_position() {
        let owner = id(1);
        let admin = id(9);
        let cache = cache_with(vec![(owner, None, false), (admin, None, true)]).await;
        assert!(cache.is_authorized_to_decide(admin, owner).await);
    }

    #[tokio::test]
    async fn owner_is_not_self_authorized() {
        let owner = id(1);
        let cache = cache_with(vec![(owner, None, false)]).await;
        assert!(!cache.is_authorized_to_decide(owner, owner).await);
    }

    #[tokio::test]
    async fn unknown_actor_is_not_authorized() {
        let owner = id(1);
        let cache = cache_with(vec![(owner, None, false)]).await;
        assert!(!cache.is_authorized_to_decide(id(99), owner).await);
    }

    #[tokio::test]
    async fn unknown_owner_is_not_authorized() {
        let actor = id(2);
        let cache = cache_with(vec![(actor, None, false)]).await;
        assert!(!cache.is_authorized_to_decide(actor, id(99)).await);
    }

    #[tokio::test]
    async fn refresh_replaces_rather_than_merges() {
        let owner = id(1);
        let old_manager = id(2);
        let cache = cache_with(vec![
            (owner, Some(old_manager), false),
            (old_manager, None, false),
        ])
        .await;
        assert!(cache.is_authorized_to_decide(old_manager, owner).await);

        let new_manager = id(3);
        cache
            .replace_from_wire(vec![
                HierarchyUserWire {
                    id: owner.to_string(),
                    parent_user_id: Some(new_manager.to_string()),
                    is_admin: false,
                },
                HierarchyUserWire {
                    id: new_manager.to_string(),
                    parent_user_id: None,
                    is_admin: false,
                },
            ])
            .await
            .unwrap();

        assert!(!cache.is_authorized_to_decide(old_manager, owner).await);
        assert!(cache.is_authorized_to_decide(new_manager, owner).await);
    }

    #[tokio::test]
    async fn load_from_json_populates_cache_from_the_same_wire_shape_the_server_sends() {
        let owner = id(1);
        let manager = id(2);
        let json = serde_json::json!([
            {"id": owner.to_string(), "parent_user_id": manager.to_string(), "is_admin": false},
            {"id": manager.to_string(), "parent_user_id": null, "is_admin": false},
        ])
        .to_string();

        let cache = HierarchyCache::new();
        cache.load_from_json(&json).await.unwrap();

        assert!(cache.is_authorized_to_decide(manager, owner).await);
    }
}
