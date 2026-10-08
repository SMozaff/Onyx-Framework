//! # worker-application
//!
//! Application-layer ports for the background worker / messaging boundary:
//! `OutboxStore`, `InboxStore`, `EventPublisher`, `DeadLetterStore`.
//!
//! Per Team Prompt 2 §2 File Manifest. Trait shapes reflect Architectural
//! Rulings (Team 2 Kickoff & Team 2 Initiation) — see `DECISIONS.md` at the
//! workspace root.

pub mod ports;

pub use ports::*;
