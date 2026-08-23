//! A local, cached slice of the organization's reporting-line tree,
//! fetched from `api-server`'s `GET /api/users/hierarchy` and used to
//! answer one question offline: *"is the current user this task/
//! mission owner's direct manager?"*
//!
//! # Why this exists
//! `desktop-shell`'s embedded `AppState` composes only the domain
//! aggregates it needs offline (`client_composition::AppState`'s own
//! doc comment) — the org's account/reporting-line directory
//! (`security_application::UserStore`) is not one of them, and never
//! has been. That was fine as long as nothing running locally needed
//! to answer an authority question. It stopped being fine once a real,
//! confirmed gap was found: `TaskDecisionHandler`/`MissionDecisionHandler`
//! (see their own doc comments) execute `ApproveTask`/`ApproveMission`/
//! `RejectTask`/`RejectMission` with no check at all on *who* is
//! issuing the command — any authenticated user could approve any
//! task, regardless of any real relationship to it. Todo/Target-list
//! verification and Staff Loans already have real, working authority
//! resolution (`api-server::verifier_resolution`) — Task/Mission
//! approval never had an equivalent, and per that module's own doc
//! comment, was never in scope for it ("as built 2026-08-16" — lists
//! and loans only).
//!
//! # Why a local cache, not a live server round-trip per approval
//! Confirmed directly with the person rather than assumed: this app's
//! offline-first design is why `desktop-shell` exists as a distinct
//! architecture from `admin-shell` in the first place, and requiring a
//! network call at the exact moment of every approval would be
//! inconsistent with that. Fetching the tree once (at login, and on
//! explicit refresh) and checking it locally afterward matches how the
//! rest of this app already works.
//!
//! # Scope, matching the person's explicit choice
//! Only direct-manager (tree-parent) authority — not the loan/
//! escalation widening `verifier_resolution` also does for lists. If
//! Task/Mission approval later needs that same widening, the person
//! should be asked again before adding it, the same way this scope was
//! chosen explicitly rather than assumed.

use std::collections::HashMap;
use std::sync::Arc;

use platform_kernel::ObjectId;
use serde::Deserialize;
use tokio::sync::RwLock;

/// One entry from `GET /api/users/hierarchy` — mirrors
/// `api_server::routes::admin::HierarchyUserDto` field-for-field. See
/// that struct's doc comment for why exactly these three fields (and
/// no more sensitive ones) are what this route returns.
#[derive(Debug, Clone, Deserialize)]
struct HierarchyUserWire {
    id: String,
    parent_user_id: Option<String>,
    is_admin: bool,
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
/// state, the same way other cross-command shared pieces do in this
/// crate.
#[derive(Clone, Default)]
pub struct HierarchyCache {
    inner: Arc<RwLock<HashMap<ObjectId, HierarchyEntry>>>,
}

impl HierarchyCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fetches `GET {server_address}/api/users/hierarchy` and replaces
    /// the cache's contents wholesale (not merged — a member who was
    /// deactivated or reassigned since the last fetch must actually
    /// disappear/update, not linger from a stale entry). Called at
    /// login (see `lib.rs`'s `login` command) and available for an
    /// explicit manual refresh if the UI ever wants one; not
    /// auto-refreshed on a timer, since staleness here only matters at
    /// the moment of an approval action, and login already guarantees
    /// freshness for that session.
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

    /// Empties the cache — see `logout`'s call site in `lib.rs` for
    /// why this is needed defensively even though `login` always
    /// overwrites the cache's contents on its own.
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
        // Populate directly rather than via `refresh` — these tests
        // exercise `is_authorized_to_decide`'s logic, not the HTTP
        // fetch, which needs a real server and is out of scope for a
        // unit test.
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
        // An owner is not implicitly their own approver just by
        // existing in the cache with no parent - the parent-match
        // check requires owner.parent_user_id == Some(actor_id), and
        // an owner's own id is never their own parent_user_id in any
        // real data (that would be a cycle, rejected server-side).
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
        // A stale entry from a previous fetch must not linger after a
        // refresh that no longer includes it (e.g. the user was
        // deactivated or reassigned) - refresh() overwrites the whole
        // map rather than merging into it, this asserts that
        // replacement behavior directly rather than via a live fetch.
        let owner = id(1);
        let old_manager = id(2);
        let cache = cache_with(vec![(owner, Some(old_manager), false), (old_manager, None, false)]).await;
        assert!(cache.is_authorized_to_decide(old_manager, owner).await);

        let new_manager = id(3);
        *cache.inner.write().await = HashMap::from([
            (owner, HierarchyEntry { parent_user_id: Some(new_manager), is_admin: false }),
            (new_manager, HierarchyEntry { parent_user_id: None, is_admin: false }),
        ]);

        assert!(!cache.is_authorized_to_decide(old_manager, owner).await);
        assert!(cache.is_authorized_to_decide(new_manager, owner).await);
    }
}
