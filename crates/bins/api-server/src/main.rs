//! ONYX integrated API server with Team 7 observability and security.

use std::net::SocketAddr;

use api_server::routes::{router, ApiState};
use observability_adapter::{init_observability, serve_metrics, ObservabilityConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_observability(&ObservabilityConfig::from_env("onyx-api-server"))?;

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite://onyx-team7.db?mode=rwc".to_string());
    let bind = std::env::var("ONYX_BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_string());
    let metrics_bind: SocketAddr = std::env::var("ONYX_METRICS_BIND")
        .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
        .parse()?;
    let storage_backend =
        if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
            "postgres"
        } else {
            "sqlite"
        };
    let state = ApiState::new(&database_url).await?;
    let metrics_task = tokio::spawn(serve_metrics(state.metrics.clone(), metrics_bind));
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, %storage_backend, %metrics_bind, "ONYX API server ready");
    let result = axum::serve(listener, app).await;
    metrics_task.abort();
    observability_adapter::shutdown_observability();
    result?;
    Ok(())
}
