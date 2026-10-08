//! # policy-domain
//!
//! ONYX Policy/LegalHold aggregates: pure domain logic for versioned
//! governance rules, deterministic policy evaluation, violations, and
//! legal hold (Part I §4.18). No I/O, no persistence, no async, and no
//! knowledge of any other domain's internals — other domains consult
//! Policy's evaluation results through the same generic authority seam
//! (`context.authority.is_authorized(...)`) they already use, per the
//! product decision recorded in `PLAN_Desktop_Web_Completion.md` §7 item 2
//! (build the full domain now, not a trimmed toggles-only subset).
//!
//! # Status
//! `Policy` and `LegalHold` are complete `AggregateRoot` implementations
//! with full state-transition test coverage — every valid transition,
//! every rejected one, both terminal states, and the
//! draft/publish/immutability invariants. Not yet wired into
//! `client-composition`'s or `api-server`'s command/query registries or
//! any persistence adapter — that integration work is tracked
//! separately (see `PLAN_Desktop_Web_Completion.md`).
//!
//! Quota & Rate Governance (§4.18.10 — `RateLimitPolicy`/`QuotaLedger`)
//! is explicitly **not** included in this crate. It is a distinct
//! aggregate pair with its own consumers (API command throttling,
//! Automation execution limits, Synchronization operation limits) that
//! no Phase 1 UI or workflow drives; adding it speculatively would be
//! unused surface area. Flagged here rather than silently omitted.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod aggregate;
pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

/// Deterministic test fixtures, exposed unconditionally for the same
/// reason as `file_domain::test_support`'s docs — other crates'
/// integration tests need to construct valid `Policy`/`LegalHold`
/// fixtures without duplicating this crate's construction logic.
pub mod test_support;

pub use aggregate::{LegalHold, Policy};
pub use command::{LegalHoldCommand, PolicyCommand};
pub use error::{LegalHoldError, PolicyError};
