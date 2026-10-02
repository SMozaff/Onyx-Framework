//! ONYX integrated API server with Team 7 observability and security.

use api_server::{
    config::AppConfig,
    routes::{router, ApiState},
};
use observability_adapter::{init_observability, serve_metrics, ObservabilityConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_observability(&ObservabilityConfig::from_env("onyx-api-server"))?;

    let config = AppConfig::from_env()?;
    let storage_backend = config.storage_backend();
    let metrics_bind = config.metrics_bind();
    let bind = config.bind();
    let state = ApiState::new_with_config(config).await?;
    let metrics_task = tokio::spawn(serve_metrics(state.metrics.clone(), metrics_bind));
    let app = router(state);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    tracing::info!(
        %bind,
        %storage_backend,
        %metrics_bind,
        "ONYX API server ready"
    );
    let result = axum::serve(listener, app).await;
    metrics_task.abort();
    observability_adapter::shutdown_observability();
    result?;
    Ok(())
}
