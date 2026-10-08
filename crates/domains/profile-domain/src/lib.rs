//! # profile-domain
//!
//! ONYX StaffProfile aggregate: pure domain logic for staff profile
//! identity, organizational info, and work-stats sections. Requested
//! 2026-08-13 — see `DECISIONS.md`'s "Staff Profiles" section for the
//! full product decisions this crate implements (public
//! identity/org-info visibility, Manager-gated work stats, admin-only
//! editing).
//!
//! # Status
//! `StaffProfile` is a complete `AggregateRoot` implementation with
//! full state-transition test coverage. Not yet wired into
//! `client-composition`'s command/query registries, `api-server`'s
//! routes, or any persistence adapter — that integration work is
//! tracked separately.
//!
//! Work-stats data has no real source yet (see
//! `value::WorkStats`'s doc comment) — this crate defines the storage
//! shape now so the rest of the profile feature is not blocked on
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` Phase D, but every
//! `WorkStats` value stored today should be
//! `WorkStats::unavailable()`.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod aggregate;
pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

/// Deterministic test fixtures, exposed unconditionally — same
/// rationale as every other domain crate's `test_support` module in
/// this workspace.
pub mod test_support;

pub use aggregate::StaffProfile;
pub use command::ProfileCommand;
pub use error::ProfileError;
pub use event::ProfileEvent;
pub use state_machine::ProfileStatus;
pub use value::{BasicIdentity, OrganizationalInfo, ProfileOwnerId, StaffProfileId, WorkStats};
