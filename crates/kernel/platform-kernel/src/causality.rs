//! Vector clock and causal relation types, used to establish a partial order
//! over events produced across replicas.

use crate::versioning::ReplicaId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A vector clock: one logical counter per known replica.
///
/// `BTreeMap` is used (not `HashMap`) so that iteration order — and therefore
/// serialization — is deterministic across processes and runs.
///
/// # JSON serialization (ruling V1 — see `DECISIONS.md`)
/// `entries` uses a custom `Serialize`/`Deserialize` (`serialize_entries`/
/// `deserialize_entries` below) that maps each `ReplicaId` key to a
/// lowercase 32-character hex string. Without this, `serde_json` rejects
/// any non-empty `VectorClock`: JSON object keys must be strings, and the
/// derived `Serialize` for `ReplicaId` (a `[u8; 16]` newtype) produces a
/// JSON array, not a string. This was a real, previously-undetected defect
/// — an empty `VectorClock` (`BTreeMap::new()`) serializes fine, which is
/// why no prior increment's tests caught it; every one of them happened to
/// only serialize the default, empty clock. See `DECISIONS.md` ruling V1
/// for the full defect report and every call site this affected.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock {
    /// Per-replica logical counters. `BTreeMap` for deterministic ordering
    /// in memory; see the struct-level doc comment for how this is
    /// actually represented on the wire.
    #[serde(
        serialize_with = "serialize_entries",
        deserialize_with = "deserialize_entries"
    )]
    pub entries: BTreeMap<ReplicaId, u64>,
}

/// Serializes `entries` as a JSON object keyed by each `ReplicaId`'s
/// lowercase 32-character hex string (via `uuid::Uuid::simple()` — reuses
/// the `uuid` crate already a dependency of this crate and already used by
/// `ReplicaId::new_random()`/its `Debug` impl, rather than adding a new
/// `hex` dependency for the same 16-byte-to-hex-string conversion; the
/// wire format is identical either way — lowercase, 32 hex characters, no
/// separators).
fn serialize_entries<S>(map: &BTreeMap<ReplicaId, u64>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::ser::SerializeMap;
    let mut ser = serializer.serialize_map(Some(map.len()))?;
    for (replica, count) in map {
        let key = uuid::Uuid::from_bytes(replica.0).simple().to_string();
        ser.serialize_entry(&key, count)?;
    }
    ser.end()
}

/// Deserializes `entries` from a JSON object keyed by lowercase
/// 32-character hex strings, the inverse of [`serialize_entries`].
fn deserialize_entries<'de, D>(deserializer: D) -> Result<BTreeMap<ReplicaId, u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct EntriesVisitor;

    impl<'de> serde::de::Visitor<'de> for EntriesVisitor {
        type Value = BTreeMap<ReplicaId, u64>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a map of 32-character lowercase hex replica ids to counters")
        }

        fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut map = BTreeMap::new();
            while let Some((key, value)) = access.next_entry::<String, u64>()? {
                let uuid = uuid::Uuid::parse_str(&key).map_err(|e| {
                    serde::de::Error::custom(format!("invalid replica id hex string {key:?}: {e}"))
                })?;
                map.insert(ReplicaId(*uuid.as_bytes()), value);
            }
            Ok(map)
        }
    }

    deserializer.deserialize_map(EntriesVisitor)
}

impl VectorClock {
    /// Creates an empty vector clock.
    pub fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Compares this clock with another, returning their causal relation.
    ///
    /// This is a strict implementation of vector-clock partial-order
    /// semantics: a replica missing from one side is treated as count `0`.
    pub fn compare(&self, other: &Self) -> CausalRelation {
        let mut self_greater = false;
        let mut other_greater = false;

        let all_replicas: BTreeSet<ReplicaId> = self
            .entries
            .keys()
            .chain(other.entries.keys())
            .cloned()
            .collect();

        for replica in all_replicas {
            let self_count = self.entries.get(&replica).copied().unwrap_or(0);
            let other_count = other.entries.get(&replica).copied().unwrap_or(0);
            if self_count > other_count {
                self_greater = true;
            }
            if other_count > self_count {
                other_greater = true;
            }
        }

        if !self_greater && !other_greater {
            CausalRelation::Equal
        } else if self_greater && !other_greater {
            CausalRelation::After
        } else if !self_greater && other_greater {
            CausalRelation::Before
        } else {
            CausalRelation::Concurrent
        }
    }

    /// Merges two clocks by componentwise maximum.
    pub fn merge(&self, other: &Self) -> Self {
        let mut merged = self.entries.clone();
        for (replica, count) in &other.entries {
            let entry = merged.entry(*replica).or_insert(0);
            *entry = (*entry).max(*count);
        }
        Self { entries: merged }
    }

    /// Increments this clock for the given replica.
    pub fn increment(&mut self, replica: ReplicaId) {
        *self.entries.entry(replica).or_insert(0) += 1;
    }

    /// Returns the count for a replica, or 0 if not present.
    pub fn get(&self, replica: &ReplicaId) -> u64 {
        self.entries.get(replica).copied().unwrap_or(0)
    }
}

impl Default for VectorClock {
    fn default() -> Self {
        Self::new()
    }
}

/// The causal relation between two vector clocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CausalRelation {
    /// This clock happened strictly before the other.
    Before,
    /// This clock happened strictly after the other.
    After,
    /// The clocks are identical.
    Equal,
    /// Neither clock happened before the other; they are concurrent.
    Concurrent,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replica(byte: u8) -> ReplicaId {
        ReplicaId([byte; 16])
    }

    #[test]
    fn empty_clocks_are_equal() {
        let a = VectorClock::new();
        let b = VectorClock::new();
        assert_eq!(a.compare(&b), CausalRelation::Equal);
    }

    #[test]
    fn strictly_greater_clock_is_after() {
        let mut a = VectorClock::new();
        a.increment(replica(1));
        let b = VectorClock::new();
        assert_eq!(a.compare(&b), CausalRelation::After);
        assert_eq!(b.compare(&a), CausalRelation::Before);
    }

    #[test]
    fn disjoint_increments_are_concurrent() {
        let mut a = VectorClock::new();
        a.increment(replica(1));
        let mut b = VectorClock::new();
        b.increment(replica(2));
        assert_eq!(a.compare(&b), CausalRelation::Concurrent);
        assert_eq!(b.compare(&a), CausalRelation::Concurrent);
    }

    #[test]
    fn merge_is_componentwise_max() {
        let mut a = VectorClock::new();
        a.increment(replica(1));
        a.increment(replica(1));
        let mut b = VectorClock::new();
        b.increment(replica(1));
        b.increment(replica(2));

        let merged = a.merge(&b);
        assert_eq!(merged.get(&replica(1)), 2);
        assert_eq!(merged.get(&replica(2)), 1);
    }

    #[test]
    fn merge_is_commutative() {
        let mut a = VectorClock::new();
        a.increment(replica(1));
        let mut b = VectorClock::new();
        b.increment(replica(2));
        b.increment(replica(2));

        assert_eq!(a.merge(&b), b.merge(&a));
    }

    #[test]
    fn increment_is_idempotent_key_insertion() {
        let mut a = VectorClock::new();
        a.increment(replica(9));
        a.increment(replica(9));
        assert_eq!(a.get(&replica(9)), 2);
        assert_eq!(a.entries.len(), 1);
    }

    #[test]
    fn get_missing_replica_returns_zero() {
        let a = VectorClock::new();
        assert_eq!(a.get(&replica(42)), 0);
    }

    // --- Ruling V1 regression tests: JSON serialization of a non-empty
    // VectorClock, which previously failed unconditionally (serde_json
    // rejects non-string map keys; the derived ReplicaId Serialize
    // produces a JSON array). See DECISIONS.md ruling V1.

    #[test]
    fn empty_vector_clock_serializes_to_json_and_back() {
        let vc = VectorClock::new();
        let json =
            serde_json::to_string(&vc).expect("empty clock always serialized, even before V1");
        let back: VectorClock = serde_json::from_str(&json).unwrap();
        assert_eq!(vc, back);
    }

    #[test]
    fn non_empty_vector_clock_serializes_to_json_and_back() {
        // This is the exact scenario that failed with "key must be a
        // string" before ruling V1's fix.
        let mut vc = VectorClock::new();
        vc.increment(replica(1));
        vc.increment(replica(1));
        vc.increment(replica(2));

        let json =
            serde_json::to_string(&vc).expect("non-empty clock must serialize per ruling V1");
        let back: VectorClock = serde_json::from_str(&json).expect("must round-trip");
        assert_eq!(vc, back);
        assert_eq!(back.get(&replica(1)), 2);
        assert_eq!(back.get(&replica(2)), 1);
    }

    #[test]
    fn vector_clock_json_keys_are_32_char_lowercase_hex() {
        let mut vc = VectorClock::new();
        let r = ReplicaId::new_random();
        vc.increment(r);

        let value = serde_json::to_value(&vc).unwrap();
        let entries = value["entries"]
            .as_object()
            .expect("entries must be a JSON object");
        assert_eq!(entries.len(), 1);
        let key = entries.keys().next().unwrap();
        assert_eq!(key.len(), 32, "key {key:?} was not 32 characters");
        assert!(
            key.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()),
            "key {key:?} was not lowercase hex"
        );
    }

    #[test]
    fn deserializing_a_malformed_key_produces_a_clear_error_not_a_panic() {
        let malformed = r#"{"entries":{"not-a-valid-hex-replica-id":1}}"#;
        let result: Result<VectorClock, _> = serde_json::from_str(malformed);
        assert!(
            result.is_err(),
            "malformed replica-id keys must be rejected, not silently accepted"
        );
    }

    #[test]
    fn many_entries_round_trip_with_correct_counts_per_replica() {
        // Guards against a subtle bug where a fixed-width/truncated
        // encoding could collide different ReplicaIds onto the same wire
        // key; 50 random ids/counts round-tripping intact rules that out.
        let mut vc = VectorClock::new();
        let mut expected: BTreeMap<ReplicaId, u64> = BTreeMap::new();
        for i in 1..=50u64 {
            let r = ReplicaId::new_random();
            for _ in 0..i {
                vc.increment(r);
            }
            expected.insert(r, i);
        }

        let json = serde_json::to_string(&vc).unwrap();
        let back: VectorClock = serde_json::from_str(&json).unwrap();
        assert_eq!(back.entries, expected);
    }
}
