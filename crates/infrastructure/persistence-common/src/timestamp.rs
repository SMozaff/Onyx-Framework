//! `platform_kernel::Timestamp` (Unix nanoseconds) <-> database column
//! (Unix milliseconds, `i64`), per Architectural Ruling "Timestamp
//! Precision (Nanoseconds -> Milliseconds)" — DECISIONS.md item T1.
//!
//! Conversion is a free function, not a `From`/`Into` impl, because
//! `Timestamp` is a foreign type (defined in `platform-kernel`) and `i64`
//! is a foreign type too — Rust's orphan rules forbid `impl From<Timestamp>
//! for i64` in this crate. Free functions avoid that restriction cleanly.

use platform_kernel::Timestamp;

/// Truncates nanoseconds to milliseconds. Sub-millisecond precision is
/// permanently and intentionally lost — see DECISIONS.md T1.
pub fn timestamp_to_millis(ts: Timestamp) -> i64 {
    (ts.0 / 1_000_000) as i64
}

/// Expands milliseconds back to nanoseconds (with trailing zeros — the
/// sub-millisecond precision lost on write can never be recovered).
pub fn millis_to_timestamp(millis: i64) -> Timestamp {
    Timestamp((millis as u64).saturating_mul(1_000_000))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_sub_millisecond_precision() {
        let ts = Timestamp(1_700_000_000_123_456_789);
        let millis = timestamp_to_millis(ts);
        assert_eq!(millis, 1_700_000_000_123);
    }

    #[test]
    fn roundtrip_loses_sub_millisecond_precision_by_design() {
        let original = Timestamp(1_700_000_000_123_456_789);
        let millis = timestamp_to_millis(original);
        let restored = millis_to_timestamp(millis);
        assert_eq!(restored.0, 1_700_000_000_123_000_000);
        assert_ne!(restored.0, original.0);
    }

    #[test]
    fn zero_roundtrips_exactly() {
        let ts = Timestamp(0);
        let millis = timestamp_to_millis(ts);
        assert_eq!(millis, 0);
        assert_eq!(millis_to_timestamp(millis).0, 0);
    }
}
