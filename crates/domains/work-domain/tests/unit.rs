//! Integration test entry point for `work-domain` unit tests.

#[path = "unit/task_creation.rs"]
mod task_creation;

#[path = "unit/task_valid_transitions.rs"]
mod task_valid_transitions;

#[path = "unit/task_invalid_transitions.rs"]
mod task_invalid_transitions;

#[path = "unit/task_guards.rs"]
mod task_guards;
