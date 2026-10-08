//! Versioning primitives.
//! Source: Handover Document §1 "versioning" (lines 64-88).

use serde::{Deserialize, Serialize};

macro_rules! u64_newtype {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        pub struct $name(pub u64);

        impl $name {
            pub const fn new(v: u64) -> Self {
                Self(v)
            }

            /// Monotonic increment. Per contract: "monotonic increasing per aggregate".
            pub fn next(self) -> Self {
                Self(self.0 + 1)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self(0)
            }
        }
    };
}

u64_newtype!(ObjectVersion, "Aggregate revision counter.");

u64_newtype!(
    LifecycleEpoch,
    "Invalidates stale lifecycle-sensitive operations."
);

u64_newtype!(AuthorityEpoch, "Invalidates stale authority assumptions.");

/// Contract/schema version for compatibility. Semver-formatted string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SchemaVersion(pub String);

impl SchemaVersion {
    pub fn new(v: impl Into<String>) -> Self {
        Self(v.into())
    }
}

/// Unique identifier for a replica device/node. UUID-formatted string.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReplicaId(pub String);

impl ReplicaId {
    pub fn new(v: impl Into<String>) -> Self {
        Self(v.into())
    }
}
