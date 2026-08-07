use std::time::Instant;

use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use observability_adapter::{emit_structured_event, StructuredLogEvent};
use platform_kernel::Timestamp;
use security_application::{RateLimitRequest, ResourceClass};
use tracing::Instrument;

use crate::routes::{authenticate_headers, parse_object_id, ApiError, ApiState};

pub async fn rate_limit_command(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Response {
    let correlation_id = request
        .headers()
        .get("x-correlation-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let authenticated = match authenticate_headers(&state, request.headers()).await {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let organization_id = match parse_object_id(&authenticated.organization_id) {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let decision = state
        .rate_limiter
        .check_and_record(&RateLimitRequest {
            organization_id,
            resource_class: ResourceClass::Commands,
            request_id: uuid::Uuid::new_v4().to_string(),
            occurred_at: Timestamp::now(),
        })
        .await;
    match decision {
        Ok(decision) if decision.allowed => next.run(request).await,
        Ok(decision) => ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "RATE_LIMITED",
            "AUTHORITY",
            "TRANSIENT",
            correlation_id,
            serde_json::json!({
                "limit": decision.limit,
                "remaining": decision.remaining,
                "reset_at_nanos": decision.reset_at.0,
            }),
        )
        .into_response(),
        Err(_error) => ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "RATE_LIMITER_UNAVAILABLE",
            "INFRASTRUCTURE",
            "TRANSIENT",
            correlation_id,
            serde_json::json!({"message": "Rate limiter unavailable"}),
        )
        .into_response(),
    }
}

pub async fn observe_request(
    State(state): State<ApiState>,
    request: Request,
    next: Next,
) -> Response {
    let started = Instant::now();
    let method = request.method().to_string();
    let route = request.uri().path().to_string();
    let trace_id = request
        .headers()
        .get("traceparent")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let span = tracing::info_span!(
        "http.request",
        service = "onyx-api-server",
        trace_id = %trace_id,
        method = %method,
        route = %route,
    );
    let response = next.run(request).instrument(span).await;
    let latency = started.elapsed();
    let status = response.status().as_u16().to_string();
    state
        .metrics
        .requests_total
        .with_label_values(&[&method, &route, &status])
        .inc();
    state
        .metrics
        .request_duration_seconds
        .with_label_values(&[&method, &route])
        .observe(latency.as_secs_f64());
    emit_structured_event(&StructuredLogEvent {
        timestamp: &Utc::now().to_rfc3339(),
        level: if response.status().is_server_error() {
            "ERROR"
        } else {
            "INFO"
        },
        service: "onyx-api-server",
        trace_id: &trace_id,
        operation_id: "",
        actor_id: "",
        organization_id: "",
        command_type: "",
        outcome: &status,
        latency_ms: latency.as_millis() as u64,
        message: "HTTP request completed",
    });
    response
}
