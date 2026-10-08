//! Time primitives.
//! Source: Handover Document §1 "time" (lines 189-203).

use serde::{Deserialize, Serialize};

/// UTC timestamp, monotonic. Unix nanoseconds since epoch.
///
/// Constraints (per contract):
/// - Must be UTC
/// - Never rely on wall clock alone for ordering
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn now() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_nanos() as u64;
        Self(nanos)
    }
}

/// Time duration, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Duration(pub u64);

impl Duration {
    pub const fn from_secs(secs: u64) -> Self {
        Self(secs * 1_000_000_000)
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self(millis * 1_000_000)
    }
}

impl From<Duration> for std::time::Duration {
    fn from(d: Duration) -> Self {
        std::time::Duration::from_nanos(d.0)
    }
}

impl From<std::time::Duration> for Duration {
    fn from(d: std::time::Duration) -> Self {
        Duration(d.as_nanos() as u64)
    }
}
