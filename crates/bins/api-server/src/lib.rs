//! `api-server` library entry point.
//!
//! # Provenance
//! `api-server` was originally delivered (Increment 2) as a `[[bin]]`-only
//! crate. Its command/query pipeline (`command_handler::handle_command`,
//! `query_handler::load_aggregate`) was therefore unreachable from any
//! other crate — including `client-composition` (Increment 5), which needs
//! to wrap it in a `CommandRegistry`/`QueryRegistry`.
//!
//! Per ruling S1 (`DECISIONS.md`), this `lib.rs` was added to expose those
//! modules as a public library, with `main.rs` consuming this crate as a
//! dependency rather than compiling the modules inline. This is a
//! packaging-only change: no logic in `command_handler.rs` or
//! `query_handler.rs` was modified, moved, or rewritten. The binary's
//! observable behavior is unchanged.

pub mod command_handler;
pub mod middleware;
pub mod query_handler;
pub mod routes;

pub use command_handler::{handle_command, CommandError, CommandResult};
pub use query_handler::load_aggregate;
