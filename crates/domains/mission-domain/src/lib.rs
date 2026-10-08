//! # mission-domain
//!
//! ONYX Mission aggregate: pure domain logic for mission lifecycle
//! management. Depends only on `platform-kernel` and `platform-contracts`.
//! No I/O, no persistence, no async.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod aggregate;
pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

/// Deterministic test fixtures. Exposed unconditionally (not gated behind
/// `#[cfg(test)]`) so that `work-domain`'s tests can also depend on them —
/// `work-domain` depends on `mission-domain` as a normal (non-dev)
/// dependency and therefore cannot see `mission-domain`'s `#[cfg(test)]`
/// items.
pub mod test_support;

pub use aggregate::Mission;
pub use command::MissionCommand;
pub use error::MissionError;
pub use event::MissionEvent;
pub use state_machine::MissionStatus;
pub use value::{MissionId, MissionSettings, MissionTimelineRef};
