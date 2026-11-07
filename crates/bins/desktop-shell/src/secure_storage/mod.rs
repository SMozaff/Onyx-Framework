//! `SecureStorage` port — frozen contract per Team Prompt 5 §3.1.
//! Copied verbatim (the prompt states "These definitions are copied
//! verbatim from the Handover Document. You MUST implement them exactly
//! as specified. No deviations are permitted.").
//!
//! # Implementation (ruling S2, `DECISIONS.md`)
//! Team Prompt 5 §2.1/§5.1 specifies three separate hand-written
//! per-OS adapter files/dependencies (`security-framework` for macOS,
//! `windows` for Windows, `secret-service` for Linux). Per ruling S2,
//! these were replaced with a single implementation
//! (`keyring_adapter.rs`) backed by the `keyring` crate (v4.x,
//! `v1` feature — its simple, direct `Entry`-based API), which wraps all
//! three platforms' native credential stores behind one safe, actively
//! maintained interface. This is a permitted implementation detail, not
//! an architectural deviation — the `SecureStorage` port itself is
//! unchanged.

pub use desktop_session::secure_storage::{SecureStorage, StorageError};

pub mod keyring_adapter;
