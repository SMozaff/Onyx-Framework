//! Shell-independent desktop session authority.
//!
//! One owner for login, refresh-token rotation, logout, and the access
//! token handed to the relay. Hosts (Tauri `desktop-shell`, native
//! `desktop-slint`) inject a `SecureStorage` adapter; this crate names no
//! Tauri, UI, or platform-keyring type.

pub mod secure_storage;
pub mod session;
pub mod session_authority;
