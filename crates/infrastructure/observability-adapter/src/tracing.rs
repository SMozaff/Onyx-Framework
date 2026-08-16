use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace as sdktrace, Resource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::CanonicalJsonLayer;

#[derive(Clone, Debug)]
pub struct ObservabilityConfig {
    pub service_name: String,
    pub otlp_endpoint: String,
    pub log_filter: String,
}

impl ObservabilityConfig {
    pub fn from_env(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
                .unwrap_or_else(|_| "http://jaeger-collector:4317".to_string()),
            log_filter: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }
}

pub fn init_observability(config: &ObservabilityConfig) -> anyhow::Result<()> {
    let tracer =
        opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(config.otlp_endpoint.clone()),
            )
            .with_trace_config(sdktrace::config().with_resource(Resource::new(vec![
                KeyValue::new("service.name", config.service_name.clone()),
            ])))
            .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
    let json = CanonicalJsonLayer::new(config.service_name.clone());
    tracing_subscriber::registry()
        .with(EnvFilter::new(config.log_filter.clone()))
        .with(telemetry)
        .with(json)
        .try_init()?;
    Ok(())
}

pub fn shutdown_observability() {
    opentelemetry::global::shutdown_tracer_provider();
}
