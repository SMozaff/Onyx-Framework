//! `sync-transport-mobile`: iOS/Android FFI bindings for the platform-native
//! Wi-Fi Direct and BLE radio stacks. See Team Prompt 4 §6.
//!
//! Per the mission statement in Team Prompt 4 §1: Team 4 exports the C-ABI
//! surface; Team 5 (Native Clients) is responsible for the actual
//! Objective-C / Kotlin/Java call sites and for wiring these exports into
//! `WifiDirectTransport`/`BluetoothLETransport`'s `PlatformHandle`.
//!
//! Compilation note (not in the prompt, called out here because it affects
//! whether `cargo build --workspace` succeeds on a non-Apple, non-Android
//! host such as the CI Linux runner used to verify this deliverable):
//! `objc` only provides a working Objective-C runtime bridge on Apple
//! targets, and `jni` expects to link against a JVM/Android NDK. Both
//! platform modules are therefore behind `target_os` cfg-gates so that
//! `cargo build`/`cargo test` succeed on Linux (where only the crate's
//! structure and non-platform-specific logic is checked), while still
//! compiling for real on `aarch64-apple-ios` / `aarch64-linux-android` per
//! the Quick Start cross-compile commands in §12. See DECISIONS.md §17.
//!
//! **Android P2P rectified (Phase 4.1, DECISIONS P2P-1):** this crate's
//! former `android_wifi_direct` / `android_ble` modules allocated phantom
//! transport handles and performed no real radio work. Per P2P-1 the
//! Android transport moved to Kotlin-owned `WifiP2pManager` /
//! `BluetoothLeScanner` drivers plus Rust-owned framing/encryption/
//! handshake in `mobile-android-jni`'s `p2p` module — so those placeholder
//! exports were **deleted, not extended**. This crate now carries only the
//! iOS native modules and the host stand-in below.

#[cfg(target_os = "ios")]
pub mod ios_multipeer;

#[cfg(target_os = "ios")]
pub mod ios_ble;

/// Non-platform-specific stand-ins compiled everywhere (including Linux
/// CI), so the crate has *something* to build/test on any host. These are
/// not part of the frozen FFI contract — they exist only so
/// `cargo test --package sync-transport-mobile` is meaningful without an
/// Apple or Android toolchain.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub mod host_stub;
