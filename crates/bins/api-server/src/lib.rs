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
//!
//! # `clippy::result_large_err` — deliberately allowed, crate-wide
//!
//! `routes::ApiError` wraps `ApiErrorBody` (3 `String`s + a `serde_json::Value` + `StatusCode`), which clippy measures at ≥136 bytes — over its 128-byte default threshold for the `Err` side of a `Result`.
//!
//! This was raised as an open question in `AUDIT_REGISTER.md` §10 (three options considered: allow, `Box<ApiError>`, or shrink `ApiErrorBody`) and resolved here as `#[allow]`, not boxing, because:
//!
//! - Every `Result<_, ApiError>` in this crate (11 call sites, all in `routes/{admin,command,events,mod}.rs`) is an internal per-request helper — constructed at most once per HTTP request on an error path, never in a loop or hot path — so the stack-copy cost clippy is warning about is immaterial here.
//! - `ApiError` is always returned via `?`/`.map_err(...)`/an explicit `Err(...)`, then immediately consumed by Axum's `IntoResponse` impl — there is no call chain where it is passed through many stack frames.
//! - Boxing would touch all 11 signatures plus every call site that constructs, matches, or destructures `ApiError`, for no measurable runtime benefit given the above. It was judged not worth the diff.
//!
//! If `ApiError` later gains large fields *and* becomes hot-path (e.g. bulk validation returning one per item in a loop), revisit this allow instead of assuming it still holds.
#![allow(clippy::result_large_err)]

pub mod command_handler;
pub mod escalation_resolution;
pub mod middleware;
pub mod query_handler;
pub mod routes;
pub mod verifier_resolution;

pub use command_handler::{handle_command, CommandError, CommandResult, OwnerAuthority, OwnerCheck};
pub use query_handler::load_aggregate;
