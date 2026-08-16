//! Shared kernel identifiers.
//! Source: Handover Document §1 "SHARED KERNEL TYPES" (lines 32-63),
//! plus Architectural Ruling (Team 2 Initiation) for *Id newtypes not
//! otherwise spelled out (OrganizationId, UserId, DeviceId, ConflictId).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! uuid_newtype {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

uuid_newtype!(
    ObjectId,
    "Globally unique, offline-generatable object identifier."
);

uuid_newtype!(OperationId, "Unique per mutation/replay operation.");

uuid_newtype!(EventId, "Unique per domain event.");

uuid_newtype!(CommandId, "Unique per command request.");

uuid_newtype!(CorrelationId, "Groups causal workflow across boundaries.");

// ---------------------------------------------------------------------
// Domain-specific identifiers.
// Per Architectural Ruling (Team 2 Initiation): these wrap ObjectId.
// ---------------------------------------------------------------------

/// Tenant/organization identifier. Wraps ObjectId per ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrganizationId(pub ObjectId);

/// User identifier. Wraps ObjectId per ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(pub ObjectId);

/// Device identifier. Wraps ObjectId per ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceId(pub ObjectId);

/// Sync/merge conflict identifier. Raw 16-byte array per ruling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConflictId(pub [u8; 16]);
