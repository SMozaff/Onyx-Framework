//! Shared test doubles/helpers for `client-composition`'s integration
//! tests (`tests/*.rs`).
//!
//! # Why this lives at `tests/support/mod.rs`, not `src/test_helpers.rs`
//! A module under `src/` is part of the **library crate**; each file
//! under `tests/` is compiled by Cargo as its own **separate crate**,
//! seeing only the library's `pub` surface. `#[cfg(test)]` only applies
//! within the library crate itself — it does not make a `src/` module
//! visible to `tests/*.rs` files either. Making a `src/test_helpers.rs`
//! module `pub` (the only way `tests/*.rs` could reach it) would ship
//! test-only scaffolding as part of the crate's real public API to every
//! downstream consumer, which isn't right either.
//!
//! `tests/support/mod.rs` is the standard, correct Rust pattern for
//! sharing code across integration test files: Cargo auto-discovers
//! every direct file under `tests/` as its own test binary, but does
//! **not** do so for files nested in a subdirectory — so
//! `tests/support/mod.rs` is invisible to Cargo's own test-binary
//! discovery and only reachable by `tests/*.rs` files that explicitly
//! `mod support;`/`#[path = "support/mod.rs"] mod support;`.
//!
//! Consolidates what were three near-identical copies of `test_pool()`
//! and two copies of `InMemoryIdempotencyStore` (in
//! `app_state_wiring.rs`, `mission_end_to_end_sqlite.rs`,
//! `task_end_to_end_sqlite.rs`) into one. No behavior change — verified
//! by running every affected test file's full suite unmodified after
//! this consolidation (see `DECISIONS.md`).

use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashMap;
use std::sync::Mutex;

/// A fresh, schema-applied, in-memory SQLite pool for one test.
/// `max_connections(1)` is required for `sqlite::memory:` specifically:
/// each additional pooled connection to an in-memory SQLite database is
/// otherwise a distinct, empty database, not a second handle to the same
/// one (this is also why `H2`'s deadlock fix in `DECISIONS.md` matters —
/// a single-connection pool is exactly the configuration that exposed
/// it).
pub async fn test_pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory SQLite database");

    // One directory deeper than the flat `tests/*.rs` files this was
    // consolidated from (`tests/support/mod.rs` vs `tests/foo.rs`), so
    // this path has one more `../` than their original copies did.
    let schema =
        include_str!("../../../../../migrations/sqlite/20260101000000_initial_schema.up.sql");
    for stmt in schema.split(';') {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }
        sqlx::query(stmt).execute(&pool).await.unwrap_or_else(|e| {
            panic!("failed to apply SQLite schema statement:\n{stmt}\n\nError: {e}")
        });
    }
    pool
}

/// In-memory `IdempotencyStore` test double. No production or mock
/// implementation of this port exists anywhere in the delivered
/// workspace (see `DECISIONS.md` ruling F1's finding) — this is
/// explicitly test-only scaffolding, not a claim that a real adapter
/// exists.
pub struct InMemoryIdempotencyStore {
    store: Mutex<HashMap<platform_kernel::OperationId, serde_json::Value>>,
}

impl InMemoryIdempotencyStore {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryIdempotencyStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Test double for `api_server::OwnerAuthority`, unconditionally
/// authorizing every actor/owner pair. None of these end-to-end tests
/// exercise the owner-gated commands (`ApproveTask`/`RejectTask`/
/// `RejectApproval`/`ActivateMission`) — they only need *some* concrete
/// `Arc<dyn OwnerAuthority>` to satisfy `TaskDecisionHandler::new`/
/// `MissionDecisionHandler::new`'s now-required parameter, added
/// alongside the real authorization fix (see `DECISIONS.md`). Allow-all
/// rather than deny-all specifically so a future test that *does* add
/// coverage for those commands isn't silently blocked by this test
/// double if it forgets to swap it out.
pub struct AllowAllOwnerAuthority;

#[async_trait::async_trait]
impl api_server::OwnerAuthority for AllowAllOwnerAuthority {
    async fn is_authorized(
        &self,
        _actor: platform_kernel::UserId,
        _owner: platform_kernel::UserId,
    ) -> bool {
        true
    }
}

#[async_trait::async_trait]
impl query_application::IdempotencyStore for InMemoryIdempotencyStore {
    async fn get(
        &self,
        operation_id: &platform_kernel::OperationId,
    ) -> Result<Option<serde_json::Value>, query_application::IdempotencyError> {
        Ok(self.store.lock().unwrap().get(operation_id).cloned())
    }

    async fn put(
        &self,
        operation_id: platform_kernel::OperationId,
        result: serde_json::Value,
    ) -> Result<(), query_application::IdempotencyError> {
        self.store.lock().unwrap().insert(operation_id, result);
        Ok(())
    }
}
