//! # todo-domain
//!
//! ONYX `TodoList`/`TargetList`/`StaffLoan` aggregates: pure domain
//! logic for Staff task lists, time-bound binary targets, list-level
//! (never item-level) verification, and time-bounded staff loans. No
//! I/O, no persistence, no async, and no knowledge of any other
//! domain's internals — same architectural posture as every other
//! `*-domain` crate in this workspace (see `policy_domain`'s crate
//! docs for the fuller rationale).
//!
//! Source: `DESIGN_User_Hierarchy_Chain_of_Authority.md` §2.1, §4,
//! §4.0.1, §4.0.1.1, §4.0.2. All product-level mechanics were resolved
//! through direct, one-at-a-time confirmation with the person (see
//! `DECISIONS.md` for the resolution history in their own words), per
//! their standing instruction to never assume or guess on undecided
//! items — no code for this crate existed before every open question
//! below was answered.
//!
//! # Status
//! `TodoList`, `TargetList`, and `StaffLoan` are complete
//! `AggregateRoot` implementations with full state-transition test
//! coverage (42 tests as of 2026-08-16). **Wired into `api-server`**
//! (not `client-composition`, which has no equivalent integration yet)
//! — full HTTP reachability over `/api/command`, `/api/query`, and
//! dedicated creation routes, live-tested end to end including
//! authorization (D.4) and read-model redaction (D.5). See
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` §11 for the complete,
//! dated account of what was built, what was verified live, and the
//! one explicit gap (the background job's SQL, never run against a
//! live Postgres).
//!
//! # What is deliberately not built in this crate
//! Everything below is intentionally kept out of this crate because it
//! is application-layer, not domain-layer, logic — but as of
//! 2026-08-16, all four now exist elsewhere in the workspace (see
//! `IMPLEMENTATION_PLAN_User_Hierarchy.md` §11 for the full account):
//! - **Verifier-resolution logic** (design doc's "who may verify a
//!   given list," combining the org tree and active staff loans) now
//!   lives in `api_server::verifier_resolution`, wired into
//!   `/api/command`'s dispatch and live-tested. Escalation widening
//!   (Phase E) remains unbuilt there too — deliberately documented as
//!   a gap, not silently stubbed. `StaffLoan::grants_verification_authority_to`
//!   (this crate) is the one piece of that logic which genuinely
//!   belongs on the aggregate itself.
//! - **Team Leader pre-check visibility redaction** (design doc §2.2)
//!   now lives in `api_server::query_handler::redact_team_leader_pre_check_for_viewer`,
//!   live-tested against a real query response. This required a fix
//!   *in this crate* first — `TodoList`/`TargetList`'s `apply()`
//!   previously discarded `TeamLeaderPreCheckRecorded`'s data entirely;
//!   see `value::TeamLeaderPreCheck` and the `team_leader_pre_check`
//!   field/accessor on both aggregates.
//! - **Escalation routing** (design doc §4.1, Phase E) — still
//!   genuinely unbuilt anywhere. `EscalateTodoList`/`EscalateTargetList`
//!   record that escalation was invoked and why; selecting the actual
//!   escalation target remains Phase E's job, not started.
//! - **The scheduled background job** for loan expiry/advance
//!   notification now lives in `worker::staff_loan_scheduler` plus two
//!   new handlers in `worker::job_runner`, compiled and clippy-clean
//!   but **not verified against a live Postgres** (none was available
//!   in the build sandbox) — see
//!   `IMPLEMENTATION_PLAN_User_Hierarchy.md` §11.3 for the explicit
//!   caveat before relying on it in production.

#![deny(warnings)]
#![deny(missing_docs)]

pub mod aggregate;
pub mod command;
pub mod error;
pub mod event;
pub mod state_machine;
pub mod value;

/// Deterministic test fixtures, exposed unconditionally so other
/// crates' integration tests can construct valid `TodoList`/
/// `TargetList`/`StaffLoan` fixtures without duplicating this crate's
/// construction logic — same rationale as `policy_domain::test_support`'s
/// docs.
pub mod test_support;

pub use aggregate::{StaffLoan, TargetList, TodoList};
pub use command::{StaffLoanCommand, TargetListCommand, TodoListCommand};
pub use error::{StaffLoanError, TargetError, TodoError};
