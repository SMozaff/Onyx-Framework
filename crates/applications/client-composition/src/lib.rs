//! `client-composition` — shared composition-root logic for ONYX's native
//! clients (`desktop-shell`, `mobile-core`).
//!
//! # Provenance
//! Team Prompt 5 (Desktop & Mobile Native Clients) §3.2/§3.4/§4.1/§4.3
//! references `CommandRegistry`, `QueryRegistry`, `EventBus`, and
//! `SyncAgent` as composition-root types each client builds, with
//! illustrative (not literal) code snippets. Neither type existed in
//! Increments 1–4; per ruling C1 (see `DECISIONS.md`), this crate holds a
//! single shared implementation that both clients wrap, rather than each
//! client duplicating ~800 lines of identical registry/dispatch logic.
//!
//! This crate depends only on the real Increment 1–4 crates
//! (`platform-kernel`, `platform-contracts`, `query-application`,
//! `mission-domain`, `work-domain`, `synchronization-domain`,
//! `sync-transport`) — no Tauri, no Flutter/FFI types. Those are the
//! respective clients' own concern.

pub mod app_state;
pub mod command_registry;
pub mod event_bus;
pub mod handlers;
pub mod query_registry;
pub mod sync_agent;

pub use app_state::{AppState, AppStateConfig};
pub use command_registry::{
    CommandDispatchError, CommandRegistry, CreationHandler, DecisionHandler,
};
pub use event_bus::{EventBus, EventFilter, EventStreamId};
pub use handlers::{
    MissionCreationHandler, MissionDecisionHandler, TaskCreationHandler, TaskDecisionHandler,
};
pub use query_registry::{QueryDispatchError, QueryHandler, QueryRegistry};
pub use sync_agent::{SyncAgent, SyncAgentConfig, SyncStatus};
