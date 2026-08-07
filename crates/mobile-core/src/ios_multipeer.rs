//! iOS Multipeer Connectivity integration — thin Rust-level wrappers
//! around `sync-transport-mobile`'s real, already-delivered
//! `extern "C"` exports (`start_multipeer_advertising`,
//! `stop_multipeer_advertising`, `ios_multipeer_connect`,
//! `ios_multipeer_send`, in `sync_transport_mobile::ios_multipeer`).
//!
//! # Why this file does not itself export new `#[no_mangle]` symbols
//! Your requested file list describes this as "iOS Multipeer
//! Connectivity FFI stubs (calls Team 4's sync-transport)" — and that is
//! exactly what this file does, but **Team 4 already built and shipped
//! the actual frozen-contract C-ABI exports** for this transport
//! directly in `sync-transport-mobile` (Team Prompt 4 §6.1), complete
//! with their own real tests. `sync-transport-mobile`'s crate-type is
//! `["cdylib", "rlib"]` — the `rlib` half means those functions are
//! callable as ordinary Rust functions from here, not just link-level
//! C symbols.
//!
//! Re-exporting or redeclaring `#[no_mangle] extern "C" fn
//! start_multipeer_advertising(...)` again in *this* crate — which also
//! builds as a `cdylib`/`staticlib` — would be a genuine **symbol
//! collision** if both crates are ever linked into the same final
//! binary (both would try to export a C symbol with the identical
//! name), not a harmless duplication. So this module calls the real
//! functions as a normal Rust dependency instead of redefining them,
//! which is both correct and is the actual "calls Team 4's
//! sync-transport" integration the file's stated purpose describes.
//!
//! # `target_os = "ios"` gating (a real constraint, discovered — not
//! assumed — by reading `sync-transport-mobile`'s own `lib.rs`)
//! `sync_transport_mobile::ios_multipeer` is itself gated behind
//! `#[cfg(target_os = "ios")]` (it depends on the `objc` crate's runtime
//! bridge, real only on Apple targets — see that crate's own module doc
//! comment and DECISIONS.md §17 there). This file mirrors that gate: on
//! any other target (including this Linux sandbox), the module compiles
//! to nothing rather than failing to find a module that genuinely does
//! not exist here. This is the exact same "typecheck-verified, not
//! linked/run against a real device" honesty Team 4 already established
//! and this crate's own module doc comment (`lib.rs`) commits to.

#![cfg(target_os = "ios")]

pub use sync_transport_mobile::ios_multipeer::{
    ios_multipeer_connect, ios_multipeer_send, start_multipeer_advertising,
    stop_multipeer_advertising,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    /// Exercises the real, already-tested `sync-transport-mobile`
    /// functions through this crate's re-export path, confirming the
    /// dependency wiring itself works (not re-testing Team 4's own
    /// logic, which already has its own test suite in that crate). Only
    /// compiled/run on iOS, per this whole file's `target_os` gate —
    /// unverifiable in this sandbox, same as every other Apple-only
    /// piece of this crate.
    #[test]
    fn multipeer_advertising_round_trip_through_mobile_core_reexport() {
        let service_type = CString::new("_onyx._tcp").unwrap();
        let handle = unsafe { start_multipeer_advertising(service_type.as_ptr()) };
        assert!(!handle.is_null());
        unsafe { stop_multipeer_advertising(handle) };
    }
}
