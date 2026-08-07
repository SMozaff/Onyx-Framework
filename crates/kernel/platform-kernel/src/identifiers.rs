//! Identifier newtypes for the ONYX platform.
//!
//! All identifiers are 16-byte arrays (UUIDv4-backed) so that serialization is
//! deterministic and binary-stable. IDs are always generated offline via
//! [`uuid::Uuid::new_v4`] — no network calls are ever required to mint one.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Generic 16-byte object identifier. Used directly for aggregate identities,
/// and wrapped by domain-specific newtypes (e.g. `MissionId`, `TaskId`).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ObjectId(pub [u8; 16]);

/// Identifies a single logical operation (a command's execution attempt).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OperationId(pub [u8; 16]);

/// Identifies a single domain event.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventId(pub [u8; 16]);

/// Identifies a single command.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CommandId(pub [u8; 16]);

/// Identifies a causal chain of commands/events across service boundaries.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CorrelationId(pub [u8; 16]);

macro_rules! impl_id_common {
    ($ty:ident) => {
        impl $ty {
            /// Generates a new random (UUIDv4-backed) identifier.
            ///
            /// This is an offline, infrastructure-free operation — no network
            /// or system calls beyond the OS random number source.
            pub fn new_random() -> Self {
                Self(*uuid::Uuid::new_v4().as_bytes())
            }

            /// Returns the raw 16-byte representation.
            pub fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }
        }

        impl fmt::Debug for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let uuid = uuid::Uuid::from_bytes(self.0);
                write!(f, concat!(stringify!($ty), "({})"), uuid)
            }
        }

        impl fmt::Display for $ty {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", uuid::Uuid::from_bytes(self.0))
            }
        }

        impl Default for $ty {
            fn default() -> Self {
                Self::new_random()
            }
        }
    };
}

impl_id_common!(ObjectId);
impl_id_common!(OperationId);
impl_id_common!(EventId);
impl_id_common!(CommandId);
impl_id_common!(CorrelationId);

/// Placeholder identifier for the (not-yet-defined) conflict-tracking
/// concept referenced by `DomainError::ConflictPending`.
///
/// # Increment 1 Ruling
/// `ConflictId` is not part of the Increment 1 domain surface — no domain
/// code in this increment constructs one. It exists solely so that
/// `DomainError::ConflictPending { conflict_id: ConflictId }` compiles.
/// Full semantics (what constitutes a conflict, how it's resolved) are
/// defined in Increment 3.
#[allow(unused)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConflictId(pub [u8; 16]);

impl_id_common!(ConflictId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_random_generation_is_unique() {
        let a = ObjectId::new_random();
        let b = ObjectId::new_random();
        assert_ne!(a, b);
    }

    #[test]
    fn object_id_ordering_is_total() {
        let a = ObjectId([0u8; 16]);
        let b = ObjectId([1u8; 16]);
        assert!(a < b);
    }

    #[test]
    fn object_id_serde_roundtrip_is_byte_array() {
        let id = ObjectId([7u8; 16]);
        let json = serde_json::to_string(&id).unwrap();
        // Must serialize as a JSON array of 16 numbers, not a string.
        assert!(json.starts_with('['));
        let back: ObjectId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn all_id_types_construct_and_debug_format() {
        let _ = format!("{:?}", OperationId::new_random());
        let _ = format!("{:?}", EventId::new_random());
        let _ = format!("{:?}", CommandId::new_random());
        let _ = format!("{:?}", CorrelationId::new_random());
    }
}
