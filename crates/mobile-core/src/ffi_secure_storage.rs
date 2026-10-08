//! Mobile `SecureStorage` — Team Prompt 5 §3.1/manifest lists this as
//! `ffi_secure_storage.rs`, "FFI secure storage (calls platform via
//! JNI/ObjC)".
//!
//! # Not implemented (flagged, not silently assumed complete)
//! A real implementation requires calling into Android's `Keystore`
//! (via JNI) or iOS's Keychain (via Objective-C/Swift interop) —
//! platform-specific FFI code this sandbox has no way to write against
//! real APIs, verify, or even typecheck meaningfully (unlike
//! `desktop-shell`'s macOS/Windows adapters, which at least link against
//! real, if unlinked/untested, Rust crates — there is no equivalent
//! "real Rust crate wrapping Android Keystore via JNI" this crate could
//! depend on and cross-compile-check the way `security-framework` was
//! for desktop macOS). Genuinely blocked on real JNI/ObjC bridge code
//! that must be written and verified against a real Android/iOS
//! toolchain, which Team 4's own precedent establishes this sandbox
//! cannot provide. No stub `SecureStorage` impl is provided here, since
//! a stub that "succeeds" without real platform-backed security would be
//! actively misleading — this is left as an explicit, visible gap
//! instead.
