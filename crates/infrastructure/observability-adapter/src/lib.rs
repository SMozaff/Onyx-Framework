//! Observability and tamper-evident audit infrastructure for Increment 7.

pub mod audit;
pub mod logging;
pub mod metrics;
pub mod tracing;

pub use audit::HashChainAuditWriter;
pub use logging::{emit_structured_event, redact_value, CanonicalJsonLayer, StructuredLogEvent};
pub use metrics::{serve_metrics, Metrics};
pub use tracing::{init_observability, shutdown_observability, ObservabilityConfig};
