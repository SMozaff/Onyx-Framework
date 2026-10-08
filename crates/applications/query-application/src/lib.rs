//! # query-application
//!
//! Application-layer ports for the command/query (write-side persistence)
//! boundary: `Repository`, `UnitOfWork`, `UnitOfWorkFactory`,
//! `IdempotencyStore`, `Connection`, `BlobStore` (Phase 1 addition — see
//! `ports::blob_store`'s doc comment).
//!
//! Per Team Prompt 2 §2 File Manifest. Trait shapes reflect Architectural
//! Rulings (Team 2 Kickoff & Team 2 Initiation) — see `DECISIONS.md` at the
//! workspace root.

pub mod ports;

pub use ports::*;
