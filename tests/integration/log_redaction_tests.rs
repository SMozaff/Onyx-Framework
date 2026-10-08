use observability_adapter::{redact_value, Metrics};

#[test]
fn secrets_are_redacted_recursively() {
    let value = serde_json::json!({
        "username":"operator",
        "password":"do-not-log",
        "nested":{"access_token":"abc","safe":"yes"},
        "items":[{"private_key":"key"}],
    });
    let encoded = redact_value(&value).to_string();
    assert!(!encoded.contains("do-not-log"));
    assert!(!encoded.contains("\"abc\""));
    assert!(!encoded.contains("\"key\""));
    assert!(encoded.contains("[REDACTED]"));
}

#[test]
fn metrics_include_required_families_and_self_metrics() {
    let metrics = Metrics::new("team7_test").unwrap();
    metrics.outbox_pending.set(1);
    metrics.job_queue_depth.set(2);
    metrics.sync_conflicts_open.set(3);
    metrics.audit_entries_total.inc();
    metrics
        .requests_total
        .with_label_values(&["GET", "/health", "200"])
        .inc();
    metrics
        .request_duration_seconds
        .with_label_values(&["GET", "/health"])
        .observe(0.01);
    let body = String::from_utf8(metrics.encode().unwrap()).unwrap();
    for family in [
        "requests_total",
        "request_duration_seconds",
        "outbox_pending",
        "job_queue_depth",
        "sync_conflicts_open",
        "audit_entries_total",
        "observability_export_errors_total",
        "observability_scrapes_total",
    ] {
        assert!(body.contains(family), "missing metric family {family}");
    }
}

#[test]
fn structured_log_schema_contains_every_required_field() {
    let event = observability_adapter::StructuredLogEvent {
        timestamp: "2026-08-05T12:00:00Z",
        level: "INFO",
        service: "api-server",
        trace_id: "trace",
        operation_id: "operation",
        actor_id: "actor",
        organization_id: "organization",
        command_type: "mission.Activate",
        outcome: "accepted",
        latency_ms: 12,
        message: "complete",
    };
    let value = serde_json::to_value(event).unwrap();
    for field in [
        "timestamp",
        "level",
        "service",
        "trace_id",
        "operation_id",
        "actor_id",
        "organization_id",
        "command_type",
        "outcome",
        "latency_ms",
    ] {
        assert!(
            value.get(field).is_some(),
            "missing structured log field {field}"
        );
    }
}
