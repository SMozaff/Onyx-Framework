//! Integration test entry point for `mission-domain` unit tests.
//! Cargo compiles each file directly under `tests/` as its own test binary;
//! this file pulls in the `unit/` submodule tree so tests can be organized
//! into multiple files while compiling as one binary (avoiding per-binary
//! `platform_kernel`/`platform_contracts` re-link overhead).

#[path = "unit/mission_creation.rs"]
mod mission_creation;

#[path = "unit/mission_valid_transitions.rs"]
mod mission_valid_transitions;

#[path = "unit/mission_invalid_transitions.rs"]
mod mission_invalid_transitions;

#[path = "unit/mission_guards.rs"]
mod mission_guards;
