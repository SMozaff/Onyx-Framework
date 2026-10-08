//! Compiled only on hosts that are neither iOS nor Android (e.g. the Linux
//! CI runner used to verify this deliverable). Not part of the frozen
//! contract — see `lib.rs` module docs for why this exists. Provides a
//! trivial, testable function so `cargo build --workspace` and
//! `cargo test --workspace` succeed and exercise at least one item on a
//! host with no Apple/Android toolchain.

/// Reports whether this build is running as the non-mobile host stub.
/// Always `true` here, since this module only compiles on non-mobile
/// targets (see the `#[cfg]` gates in `lib.rs`).
pub fn is_mobile_host_stub() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_stub_reports_stub_status() {
        assert!(is_mobile_host_stub());
    }
}
