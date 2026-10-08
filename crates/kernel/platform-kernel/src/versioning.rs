//! Versioning and epoch types.
//!
//! `ObjectVersion` tracks optimistic-concurrency state per aggregate.
//! `LifecycleEpoch` and `AuthorityEpoch` are coarser-grained monotonic
//! counters that advance on lifecycle/authority-changing events respectively;
//! they are set externally (there is no `next()` — see Implementation Notes
//! in the frozen contract, §3.2).

use serde::{Deserialize, Serialize};

/// Monotonically increasing per-aggregate version, incremented once per
/// accepted event.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectVersion(pub u64);

impl ObjectVersion {
    /// The initial version of a freshly created aggregate.
    pub const INITIAL: ObjectVersion = ObjectVersion(0);

    /// Returns the next version in sequence.
    pub fn next(self) -> Self {
        ObjectVersion(self.0 + 1)
    }
}

impl Default for ObjectVersion {
    fn default() -> Self {
        Self::INITIAL
    }
}

/// Advances on lifecycle-changing commands (activate, pause, halt, close,
/// etc.). Set externally; no `next()` is provided on the type itself because
/// the decision of *whether* a given event advances the epoch is a domain
/// concern, not a kernel concern.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleEpoch(pub u64);

impl LifecycleEpoch {
    /// The initial lifecycle epoch of a freshly created aggregate.
    pub const INITIAL: LifecycleEpoch = LifecycleEpoch(0);

    /// Advances the epoch by one. Domain code calls this explicitly when a
    /// lifecycle-changing event is applied.
    pub fn advance(self) -> Self {
        LifecycleEpoch(self.0 + 1)
    }
}

impl Default for LifecycleEpoch {
    fn default() -> Self {
        Self::INITIAL
    }
}

/// Advances when authority over an aggregate changes (e.g. owner change).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityEpoch(pub u64);

impl AuthorityEpoch {
    /// The initial authority epoch of a freshly created aggregate.
    pub const INITIAL: AuthorityEpoch = AuthorityEpoch(0);

    /// Advances the epoch by one. Domain code calls this explicitly when an
    /// authority-changing event is applied.
    pub fn advance(self) -> Self {
        AuthorityEpoch(self.0 + 1)
    }
}

impl Default for AuthorityEpoch {
    fn default() -> Self {
        Self::INITIAL
    }
}

/// A semantic schema version string, e.g. `"1.0.0"`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaVersion(pub String);

impl SchemaVersion {
    /// Constructs a `SchemaVersion` from any string-like value.
    pub fn new(version: impl Into<String>) -> Self {
        Self(version.into())
    }
}

/// Identifies a replica participating in the vector-clock causality scheme.
/// Uses the same UUIDv4 generation strategy as [`crate::identifiers::ObjectId`].
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReplicaId(pub [u8; 16]);

impl ReplicaId {
    /// Generates a new random (UUIDv4-backed) replica identifier.
    pub fn new_random() -> Self {
        Self(*uuid::Uuid::new_v4().as_bytes())
    }
}

impl std::fmt::Debug for ReplicaId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReplicaId({})", uuid::Uuid::from_bytes(self.0))
    }
}

impl Default for ReplicaId {
    fn default() -> Self {
        Self::new_random()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_version_next_increments_by_one() {
        let v = ObjectVersion(5);
        assert_eq!(v.next(), ObjectVersion(6));
    }

    #[test]
    fn object_version_initial_is_zero() {
        assert_eq!(ObjectVersion::INITIAL, ObjectVersion(0));
        assert_eq!(ObjectVersion::default(), ObjectVersion(0));
    }

    #[test]
    fn lifecycle_epoch_advances() {
        let e = LifecycleEpoch::INITIAL;
        assert_eq!(e.advance(), LifecycleEpoch(1));
    }

    #[test]
    fn authority_epoch_advances() {
        let e = AuthorityEpoch::INITIAL;
        assert_eq!(e.advance(), AuthorityEpoch(1));
    }

    #[test]
    fn replica_id_random_generation_is_unique() {
        let a = ReplicaId::new_random();
        let b = ReplicaId::new_random();
        assert_ne!(a, b);
    }

    #[test]
    fn schema_version_holds_string() {
        let v = SchemaVersion::new("1.0.0");
        assert_eq!(v.0, "1.0.0");
    }
}
