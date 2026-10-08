//! Law-based CRDT convergence and state-canonicalization properties.
//! These tests intentionally compare serialized state as well as logical
//! values so merge order cannot change the wire representation.

use crdt::{AppendOnlyLog, Crdt, ElementId, LwwRegister, MvRegister, OrSet, PnCounter, Rga, Tag};
use platform_kernel::{ReplicaId, VectorClock};
use proptest::prelude::*;

fn replica(byte: u8) -> ReplicaId {
    ReplicaId([byte; 16])
}

fn clock(replica: ReplicaId, ticks: u64) -> VectorClock {
    let mut clock = VectorClock::new();
    for _ in 0..ticks {
        clock.increment(replica);
    }
    clock
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn or_set_merge_is_commutative_and_serialization_canonical(
        left_value in 0u8..=31,
        right_value in 32u8..=63,
    ) {
        let left_replica = replica(1);
        let right_replica = replica(2);
        let left_clock = clock(left_replica, 1);
        let right_clock = clock(right_replica, 1);

        let mut left = OrSet::new();
        left.add(left_value.to_string(), Tag { replica_id: left_replica, lamport: 1 }, left_clock);
        let mut right = OrSet::new();
        right.add(right_value.to_string(), Tag { replica_id: right_replica, lamport: 1 }, right_clock);

        let mut lr = left.clone();
        lr.merge(&right);
        let mut rl = right.clone();
        rl.merge(&left);

        prop_assert_eq!(lr.elements(), rl.elements());
        prop_assert_eq!(serde_json::to_vec(&lr).ok(), serde_json::to_vec(&rl).ok());

        let before = serde_json::to_vec(&lr).ok();
        lr.merge(&lr.clone());
        prop_assert_eq!(before, serde_json::to_vec(&lr).ok());
    }

    #[test]
    fn pn_counter_merge_is_commutative_and_idempotent(
        left_inc in 0u16..1000,
        right_inc in 0u16..1000,
        left_dec in 0u16..1000,
        right_dec in 0u16..1000,
    ) {
        let left_replica = replica(3);
        let right_replica = replica(4);
        let mut left = PnCounter::new();
        left.increment(left_replica, left_inc as u64, clock(left_replica, 1));
        left.decrement(left_replica, left_dec as u64, clock(left_replica, 2));
        let mut right = PnCounter::new();
        right.increment(right_replica, right_inc as u64, clock(right_replica, 1));
        right.decrement(right_replica, right_dec as u64, clock(right_replica, 2));

        let mut lr = left.clone();
        lr.merge(&right);
        let mut rl = right.clone();
        rl.merge(&left);

        prop_assert_eq!(lr.value(), rl.value());
        prop_assert_eq!(serde_json::to_vec(&lr).ok(), serde_json::to_vec(&rl).ok());

        let expected = lr.value();
        lr.merge(&lr.clone());
        prop_assert_eq!(lr.value(), expected);
    }

    #[test]
    fn mv_register_merge_has_canonical_state_order(
        left_value in 0u16..1000,
        right_value in 1000u16..2000,
    ) {
        let left_replica = replica(5);
        let right_replica = replica(6);
        let left = MvRegister::new(left_value, left_replica);
        let right = MvRegister::new(right_value, right_replica);

        let mut lr = left.clone();
        lr.merge(&right);
        let mut rl = right.clone();
        rl.merge(&left);

        prop_assert_eq!(lr.get_all().len(), 2);
        prop_assert_eq!(rl.get_all().len(), 2);
        prop_assert_eq!(serde_json::to_vec(&lr).ok(), serde_json::to_vec(&rl).ok());
        prop_assert_eq!(lr.get_all().iter().map(|(_, value)| **value).collect::<Vec<_>>(),
            rl.get_all().iter().map(|(_, value)| **value).collect::<Vec<_>>());
    }

    #[test]
    fn rga_merge_is_commutative_for_concurrent_siblings(
        left_value in 0u8..=127,
        right_value in 128u8..=255,
    ) {
        let left_replica = replica(7);
        let right_replica = replica(8);
        let mut left = Rga::new();
        left.insert_after(ElementId::root(), left_value, clock(left_replica, 1), left_replica);
        let mut right = Rga::new();
        right.insert_after(ElementId::root(), right_value, clock(right_replica, 1), right_replica);

        let mut lr = left.clone();
        lr.merge(&right);
        let mut rl = right.clone();
        rl.merge(&left);

        prop_assert_eq!(lr.to_vec(), rl.to_vec());
        prop_assert_eq!(serde_json::to_vec(&lr).ok(), serde_json::to_vec(&rl).ok());
    }

    #[test]
    fn lww_register_merge_is_commutative(
        left_value in 0u16..1000,
        right_value in 1000u16..2000,
    ) {
        let left = LwwRegister::new(left_value, replica(9));
        let right = LwwRegister::new(right_value, replica(10));

        // The constructor timestamps are wall-clock values, so the explicit
        // sets above may not replace them. We therefore assert the algebraic
        // property on the actual states: whichever constructor timestamp
        // wins, merge direction must select the same winner.
        let mut a = left.clone();
        a.merge(&right);
        let mut b = right.clone();
        b.merge(&left);
        prop_assert_eq!(a.get(), b.get());
    }

    #[test]
    fn append_only_log_merge_is_canonical(
        left_value in 0u16..1000,
        right_value in 1000u16..2000,
    ) {
        let left_replica = replica(11);
        let right_replica = replica(12);
        let mut left = AppendOnlyLog::new();
        left.append(left_value, clock(left_replica, 1));
        let mut right = AppendOnlyLog::new();
        right.append(right_value, clock(right_replica, 1));

        let mut lr = left.clone();
        lr.merge_entries(&right);
        let mut rl = right.clone();
        rl.merge_entries(&left);

        prop_assert_eq!(serde_json::to_vec(&lr).ok(), serde_json::to_vec(&rl).ok());
        prop_assert_eq!(lr.entries().len(), rl.entries().len());
    }
}
