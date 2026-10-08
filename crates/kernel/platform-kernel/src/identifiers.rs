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

            /// Parses an identifier from its canonical UUID string form
            /// (e.g. `"550e8400-e29b-41d4-a716-446655440000"`), the same
            /// textual form `Display` produces and the same form
            /// `api-server`'s JSON responses use for every id field.
            ///
            /// Added for `desktop-shell`'s real login flow: `/api/auth/login`
            /// returns `organization_id`/`id` as plain UUID strings (see
            /// `api-server::routes::auth::LoginUser`), and there was
            /// previously no way to turn that string back into this crate's
            /// 16-byte id types — only `new_random()` existed. Without this,
            /// the server's real organization id could never be used to
            /// construct an `OrganizationId` on the client.
            pub fn from_uuid_str(s: &str) -> Result<Self, uuid::Error> {
                Ok(Self(*uuid::Uuid::parse_str(s)?.as_bytes()))
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

        impl std::str::FromStr for $ty {
            type Err = uuid::Error;

            /// Enables `str.parse::<ObjectId>()` / `"...".parse()`, the
            /// idiomatic Rust entry point — implemented in terms of
            /// `from_uuid_str` above so there is exactly one parsing rule.
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_uuid_str(s)
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
    fn object_id_display_and_from_uuid_str_round_trip() {
        // The exact scenario this exists for: api-server sends
        // organization_id as a canonical UUID string; the client must
        // be able to turn it back into a real ObjectId.
        let original = ObjectId::new_random();
        let text = original.to_string();
        let parsed = ObjectId::from_uuid_str(&text).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn object_id_from_str_trait_matches_from_uuid_str() {
        // FromStr must behave identically to from_uuid_str, not a
        // second, subtly different parsing rule.
        let text = "550e8400-e29b-41d4-a716-446655440000";
        let via_method = ObjectId::from_uuid_str(text).unwrap();
        let via_trait: ObjectId = text.parse().unwrap();
        assert_eq!(via_method, via_trait);
    }

    #[test]
    fn object_id_from_uuid_str_rejects_garbage() {
        assert!(ObjectId::from_uuid_str("not-a-uuid").is_err());
        assert!(ObjectId::from_uuid_str("").is_err());
    }

    #[test]
    fn all_id_types_construct_and_debug_format() {
        let _ = format!("{:?}", OperationId::new_random());
        let _ = format!("{:?}", EventId::new_random());
        let _ = format!("{:?}", CommandId::new_random());
        let _ = format!("{:?}", CorrelationId::new_random());
    }
}
