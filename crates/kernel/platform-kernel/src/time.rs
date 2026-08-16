//! Time types. `Timestamp` is UTC, expressed as Unix nanoseconds.
//!
//! Vector clocks — not wall-clock time — are authoritative for causal
//! ordering. `Timestamp` exists for human-readable audit trails and
//! epoch-style deadlines, never for ordering events across replicas.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// A UTC timestamp, expressed as nanoseconds since the Unix epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// Returns the current system time as a `Timestamp`.
    ///
    /// # Panics
    /// Panics if the system clock is set before the Unix epoch. This is the
    /// one intentional exception to the "no panics" rule (§10 acceptance
    /// criteria refers to production *domain logic*; this is a kernel
    /// time-source primitive where a pre-1970 clock indicates a
    /// fundamentally broken host environment).
    pub fn now() -> Self {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is set before the Unix epoch");
        Self(duration.as_nanos() as u64)
    }

    /// Constructs a `Timestamp` from a raw nanosecond-since-epoch value.
    /// Primarily for tests and deterministic fixtures.
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// Adds a `Duration`, returning `None` on overflow.
    pub fn checked_add(&self, duration: Duration) -> Option<Self> {
        self.0.checked_add(duration.0).map(Timestamp)
    }
}

/// A duration in nanoseconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Duration(pub u64);

impl Duration {
    /// Constructs a `Duration` from a raw nanosecond value.
    pub const fn from_nanos(nanos: u64) -> Self {
        Self(nanos)
    }

    /// Constructs a `Duration` from a whole number of seconds.
    pub const fn from_secs(secs: u64) -> Self {
        Self(secs.saturating_mul(1_000_000_000))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_nonzero_and_monotonic_enough() {
        let a = Timestamp::now();
        let b = Timestamp::now();
        assert!(a.0 > 0);
        assert!(b >= a);
    }

    #[test]
    fn checked_add_succeeds_within_range() {
        let t = Timestamp::from_nanos(100);
        let d = Duration::from_nanos(50);
        assert_eq!(t.checked_add(d), Some(Timestamp::from_nanos(150)));
    }

    #[test]
    fn checked_add_overflows_to_none() {
        let t = Timestamp::from_nanos(u64::MAX);
        let d = Duration::from_nanos(1);
        assert_eq!(t.checked_add(d), None);
    }

    #[test]
    fn duration_from_secs_converts_to_nanos() {
        assert_eq!(Duration::from_secs(1), Duration::from_nanos(1_000_000_000));
    }
}
