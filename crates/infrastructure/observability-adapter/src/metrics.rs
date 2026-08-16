use std::{collections::HashMap, net::SocketAddr};

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use prometheus::{
    Encoder, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge, Opts, Registry,
    TextEncoder,
};

#[derive(Clone)]
pub struct Metrics {
    registry: Registry,
    pub requests_total: IntCounterVec,
    pub request_duration_seconds: HistogramVec,
    pub outbox_pending: IntGauge,
    pub job_queue_depth: IntGauge,
    pub sync_conflicts_open: IntGauge,
    pub audit_entries_total: IntCounter,
    pub observability_export_errors_total: IntCounter,
    pub observability_scrapes_total: IntCounter,
}

impl Metrics {
    pub fn new(service: &str) -> Result<Self, prometheus::Error> {
        let registry = Registry::new_custom(
            None,
            Some(HashMap::from([(
                "service".to_string(),
                service.to_string(),
            )])),
        )?;
        let requests_total = IntCounterVec::new(
            Opts::new("requests_total", "Total HTTP requests"),
            &["method", "route", "status"],
        )?;
        let request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "request_duration_seconds",
                "HTTP request latency in seconds",
            ),
            &["method", "route"],
        )?;
        let outbox_pending = IntGauge::new("outbox_pending", "Pending outbox messages")?;
        let job_queue_depth = IntGauge::new("job_queue_depth", "Pending/runnable jobs")?;
        let sync_conflicts_open =
            IntGauge::new("sync_conflicts_open", "Open synchronization conflicts")?;
        let audit_entries_total = IntCounter::new("audit_entries_total", "Audit entries appended")?;
        let observability_export_errors_total = IntCounter::new(
            "observability_export_errors_total",
            "Failures emitted by the observability stack itself",
        )?;
        let observability_scrapes_total = IntCounter::new(
            "observability_scrapes_total",
            "Successful Prometheus scrapes",
        )?;

        registry.register(Box::new(requests_total.clone()))?;
        registry.register(Box::new(request_duration_seconds.clone()))?;
        registry.register(Box::new(outbox_pending.clone()))?;
        registry.register(Box::new(job_queue_depth.clone()))?;
        registry.register(Box::new(sync_conflicts_open.clone()))?;
        registry.register(Box::new(audit_entries_total.clone()))?;
        registry.register(Box::new(observability_export_errors_total.clone()))?;
        registry.register(Box::new(observability_scrapes_total.clone()))?;

        Ok(Self {
            registry,
            requests_total,
            request_duration_seconds,
            outbox_pending,
            job_queue_depth,
            sync_conflicts_open,
            audit_entries_total,
            observability_export_errors_total,
            observability_scrapes_total,
        })
    }

    pub fn encode(&self) -> Result<Vec<u8>, prometheus::Error> {
        let families = self.registry.gather();
        let mut buffer = Vec::new();
        TextEncoder::new().encode(&families, &mut buffer)?;
        Ok(buffer)
    }
}

async fn metrics_handler(State(metrics): State<Metrics>) -> impl IntoResponse {
    match metrics.encode() {
        Ok(body) => {
            metrics.observability_scrapes_total.inc();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, TextEncoder::new().format_type())],
                body,
            )
                .into_response()
        }
        Err(error) => {
            metrics.observability_export_errors_total.inc();
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                format!("metrics encoding failed: {error}"),
            )
                .into_response()
        }
    }
}

pub async fn serve_metrics(metrics: Metrics, bind: SocketAddr) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(metrics);
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
