use std::time::{Duration, Instant};

use api_server::routes::{router, ApiState};
use axum::{body::Body, http::Request};
use tower::ServiceExt;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn in_process_http_p95_is_below_500ms() -> anyhow::Result<()> {
    std::env::set_var("ONYX_ENV", "test");
    let app = router(ApiState::new("sqlite::memory:").await?);
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..100 {
        let service = app.clone();
        tasks.spawn(async move {
            let started = Instant::now();
            let response = service
                .oneshot(
                    Request::builder()
                        .uri("/health")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(response.status().is_success());
            started.elapsed()
        });
    }
    let mut samples = Vec::new();
    while let Some(sample) = tasks.join_next().await {
        samples.push(sample?);
    }
    samples.sort_unstable();
    let p95_index = ((samples.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    let p95 = samples[p95_index];
    assert!(p95 < Duration::from_millis(500), "p95 was {p95:?}");
    Ok(())
}
