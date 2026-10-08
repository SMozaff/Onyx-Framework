use std::sync::OnceLock;

use opentelemetry::{global, trace::TracerProvider as _, KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
use opentelemetry_sdk::{trace as sdktrace, Resource};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::CanonicalJsonLayer;

static TRACER_PROVIDER: OnceLock<sdktrace::SdkTracerProvider> = OnceLock::new();

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
            // OTLP export is opt-in. Deployments without a collector must not
            // resolve a development/Kubernetes hostname such as jaeger-collector.
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_default(),
            log_filter: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }
}

pub fn init_observability(config: &ObservabilityConfig) -> anyhow::Result<()> {
    // OTLP/gRPC, not OTLP/HTTP: every deployment manifest (deploy/helm/*,
    // deploy/docker-compose.local.yml) points OTEL_EXPORTER_OTLP_ENDPOINT at
    // the collector's gRPC port 4317, and docker-compose publishes only 4317.
    // An HTTP exporter here silently exported to a port nothing listens on,
    // dropping every span batch while looking healthy.
    let json = CanonicalJsonLayer::new(config.service_name.clone());
    let registry = tracing_subscriber::registry()
        .with(EnvFilter::new(config.log_filter.clone()))
        .with(json);

    if config.otlp_endpoint.trim().is_empty() {
        // Structured logging remains enabled when no OTLP collector is
        // configured. This is the expected composition for Render until an
        // actual managed OTLP endpoint is provisioned.
        registry.try_init()?;
    } else {
        let exporter = SpanExporter::builder()
            .with_tonic()
            .with_endpoint(config.otlp_endpoint.clone())
            .build()?;

        let resource = Resource::builder()
            .with_service_name(config.service_name.clone())
            .with_attributes([KeyValue::new("service.name", config.service_name.clone())])
            .build();
        let provider = sdktrace::SdkTracerProvider::builder()
            .with_batch_exporter(exporter)
            .with_resource(resource)
            .build();
        let tracer = provider.tracer("onyx-observability");
        let _ = TRACER_PROVIDER.set(provider.clone());
        global::set_tracer_provider(provider);

        let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        registry.with(telemetry).try_init()?;
    };
    Ok(())
}

pub fn shutdown_observability() {
    if let Some(provider) = TRACER_PROVIDER.get() {
        let _ = provider.shutdown();
    }
}
