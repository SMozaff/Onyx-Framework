//! ONYX production background worker: outbox relay, durable job runner,
//! five-second timeline scheduler, hourly snapshotter, WebPush delivery, and
//! Prometheus/OTLP observability.

use std::{net::SocketAddr, sync::Arc, time::Duration};

use background_jobs::PostgresJobQueue;
use messaging_adapter::ChannelEventPublisher;
use observability_adapter::{init_observability, serve_metrics, Metrics, ObservabilityConfig};
use persistence_postgres::{PostgresDeadLetterStore, PostgresOutboxStore};
use sqlx::postgres::PgPoolOptions;
use worker::job_runner::JobRunnerConfig;
use worker::push_delivery::HttpPushSender;
use worker::webpush::VapidSigner;
use worker_application::{JobQueue, OutboxStore};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let observability = ObservabilityConfig::from_env("onyx-worker");
    init_observability(&observability)?;
    let metrics = Metrics::new("onyx_worker")?;

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for the production worker");
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(&database_url)
        .await?;
    sqlx::migrate!("../../../migrations/postgres")
        .run(&pool)
        .await?;

    let outbox_store: Arc<dyn OutboxStore> = Arc::new(PostgresOutboxStore::new(pool.clone()));
    let publisher = Arc::new(ChannelEventPublisher::new());
    let dead_letter = Arc::new(PostgresDeadLetterStore::new(pool.clone()));
    let job_queue: Arc<dyn JobQueue> = Arc::new(PostgresJobQueue::new(pool.clone()));

    let metrics_bind: SocketAddr = std::env::var("ONYX_METRICS_BIND")
        .unwrap_or_else(|_| "0.0.0.0:9090".to_string())
        .parse()?;
    let metrics_task = tokio::spawn(serve_metrics(metrics.clone(), metrics_bind));

    let outbox_task = tokio::spawn(worker::outbox_relay::run_outbox_relay(
        Arc::clone(&outbox_store),
        publisher,
        dead_letter,
        worker::outbox_relay::RelayConfig::default(),
    ));
    let jobs_task = tokio::spawn(worker::job_runner::run_job_runner(
        Arc::clone(&job_queue),
        pool.clone(),
        metrics.clone(),
        JobRunnerConfig::default(),
    ));
    let scheduler_task = tokio::spawn(worker::scheduler_loop::run_scheduler(
        Arc::clone(&job_queue),
        pool.clone(),
    ));
    // StaffLoan advance-warning/expiry scan (2026-08-16) — see
    // worker::staff_loan_scheduler's module doc comment and
    // IMPLEMENTATION_PLAN_User_Hierarchy.md C.3.
    let staff_loan_scheduler_task =
        tokio::spawn(worker::staff_loan_scheduler::run_staff_loan_scheduler(
            Arc::clone(&job_queue),
            pool.clone(),
        ));
    let snapshot_task = tokio::spawn(worker::snapshot_loop::run_snapshotter(pool.clone()));

    // WebPush delivery (PWA ObserverClient notifications). VAPID config is
    // read from the environment; the public key must match the `k` value the
    // PWA stub-verifies against (mobile-pwa `tests/browser/push.spec.ts`).
    let vapid_private_pkcs8_b64 = std::env::var("ONYX_VAPID_PRIVATE_KEY_PKCS8_BASE64")
        .expect("ONYX_VAPID_PRIVATE_KEY_PKCS8_BASE64 must be set for the push delivery worker");
    let vapid_pkcs8 = worker::webpush::b64url_decode(&vapid_private_pkcs8_b64)
        .expect("ONYX_VAPID_PRIVATE_KEY_PKCS8_BASE64 must be valid base64url");
    let vapid_signer = VapidSigner::from_pkcs8_der(&vapid_pkcs8)
        .expect("ONYX_VAPID_PRIVATE_KEY_PKCS8_BASE64 must be a P-256 PKCS#8 DER key");
    let push_delivery_config = worker::push_delivery::PushDeliveryConfig::from_env();
    let push_sender =
        Arc::new(HttpPushSender::new(push_delivery_config.http_timeout).expect("push HTTP client"));
    tracing::info!(
        vapid_public_key = %vapid_signer.public_key_base64url(),
        push_delivery_config = "loaded",
        "WebPush delivery configured"
    );
    let push_delivery_task = tokio::spawn(worker::push_delivery::run_push_delivery(
        pool.clone(),
        push_delivery_config.clone(),
        Arc::new(vapid_signer),
        push_sender,
    ));

    let outbox_metrics = {
        let metrics = metrics.clone();
        let outbox_store = Arc::clone(&outbox_store);
        tokio::spawn(async move {
            loop {
                match outbox_store.pending_count().await {
                    Ok(count) => metrics.outbox_pending.set(count as i64),
                    Err(_error) => {
                        metrics.observability_export_errors_total.inc();
                        tracing::warn!(
                            error_class = "outbox_metric_refresh",
                            "failed to refresh outbox_pending metric"
                        );
                    }
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        })
    };

    tracing::info!(metrics_bind = %metrics_bind, "ONYX worker started");
    tokio::signal::ctrl_c().await?;
    tracing::info!("worker shutdown requested");

    metrics_task.abort();
    outbox_task.abort();
    jobs_task.abort();
    scheduler_task.abort();
    staff_loan_scheduler_task.abort();
    snapshot_task.abort();
    push_delivery_task.abort();
    outbox_metrics.abort();
    observability_adapter::shutdown_observability();
    Ok(())
}
