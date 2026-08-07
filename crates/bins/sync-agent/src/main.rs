//! ONYX headless synchronization-agent process.
//!
//! Increment 4 delivered transport and reconciliation libraries but no
//! process-level composition root. Team 8 supplies the release process shell:
//! observability, lifecycle, health heartbeat and configuration validation.
//! Transport-specific adapters remain selected by the native clients.

use std::{net::SocketAddr, time::Duration};

use observability_adapter::{init_observability, serve_metrics, Metrics, ObservabilityConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_observability(&ObservabilityConfig::from_env("onyx-sync-agent"))?;
    let metrics = Metrics::new("onyx_sync_agent")?;
    let metrics_bind: SocketAddr = std::env::var("ONYX_METRICS_BIND")
        .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
        .parse()?;
    let relay_url = std::env::var("ONYX_CLOUD_RELAY_URL")
        .unwrap_or_else(|_| "wss://relay.invalid/api/sync".to_string());
    let metrics_task = tokio::spawn(serve_metrics(metrics.clone(), metrics_bind));

    tracing::info!(%relay_url, %metrics_bind, "ONYX sync agent started");
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                tracing::info!(%relay_url, "sync agent heartbeat");
            }
            result = tokio::signal::ctrl_c() => {
                result?;
                break;
            }
        }
    }

    metrics_task.abort();
    observability_adapter::shutdown_observability();
    Ok(())
}
