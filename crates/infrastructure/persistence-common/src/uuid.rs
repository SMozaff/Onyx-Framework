//! `[u8; 16]`-backed identifier <-> database `BLOB`/`UUID` column helpers.
//!
//! IMPORTANT: Team 1's real `platform-kernel` identifiers (`ObjectId`,
//! `EventId`, `OperationId`, `CommandId`, `CorrelationId`) are backed by a
//! raw `[u8; 16]`, NOT `uuid::Uuid`. This is simpler than originally
//! assumed in the pre-Team-1 stub (which wrapped `uuid::Uuid`) — no `uuid`
//! crate round-trip is needed for SQLite `BLOB` storage; `[u8; 16]` is
//! already the exact byte layout SQLite/Postgres `BYTEA`/`UUID` want.
//!
//! For PostgreSQL's native `UUID` column type, sqlx's `uuid` feature maps
//! to `uuid::Uuid`, so a conversion through `uuid::Uuid::from_bytes` /
//! `.into_bytes()` is still needed at the Postgres adapter boundary
//! specifically (see `persistence-postgres`). SQLite's `BLOB` column takes
//! `&[u8]` directly with no intermediate type.

use uuid::Uuid;

/// Convert a 16-byte identifier array to a `uuid::Uuid`, for binding into
/// a PostgreSQL `UUID` column via sqlx.
pub fn bytes_to_uuid(bytes: [u8; 16]) -> Uuid {
    Uuid::from_bytes(bytes)
}

/// Convert a `uuid::Uuid` (as read back from a PostgreSQL `UUID` column)
/// into the raw 16-byte array our kernel identifiers use.
pub fn uuid_to_bytes(uuid: Uuid) -> [u8; 16] {
    *uuid.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_exactly() {
        let original = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let uuid = bytes_to_uuid(original);
        let restored = uuid_to_bytes(uuid);
        assert_eq!(original, restored);
    }
}
