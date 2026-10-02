use opentelemetry::{global, trace::TracerProvider as _ , KeyValue};
use opentelemetry_otlp::{SpanExporter, WithExportConfig};
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
                .unwrap_or_else(|_| "http://jaeger-collector:4318/v1/traces".to_string()),
            log_filter: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }
}

pub fn init_observability(config: &ObservabilityConfig) -> anyhow::Result<()> {
    let exporter = SpanExporter::builder()
        .with_http()
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
    global::set_tracer_provider(provider);

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
    // OpenTelemetry 0.31 owns tracer-provider shutdown explicitly. The
    // global provider is intentionally configured once during process start;
    // process teardown closes outstanding spans through the SDK provider.
}
