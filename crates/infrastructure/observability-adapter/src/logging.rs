use std::{fmt, io::Write};

use chrono::{SecondsFormat, Utc};
use opentelemetry::trace::TraceContextExt;
use serde::Serialize;
use serde_json::{json, Map, Value};
use tracing::{
    field::{Field, Visit},
    Event, Subscriber,
};
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer};

/// Canonical structured-log payload. Callers must pass only redacted values.
#[derive(Clone, Debug, Serialize)]
pub struct StructuredLogEvent<'a> {
    pub timestamp: &'a str,
    pub level: &'a str,
    pub service: &'a str,
    pub trace_id: &'a str,
    pub operation_id: &'a str,
    pub actor_id: &'a str,
    pub organization_id: &'a str,
    pub command_type: &'a str,
    pub outcome: &'a str,
    pub latency_ms: u64,
    pub message: &'a str,
}

pub fn emit_structured_event(event: &StructuredLogEvent<'_>) {
    match event.level {
        "ERROR" | "error" => tracing::error!(
            timestamp = event.timestamp,
            service = event.service,
            trace_id = event.trace_id,
            operation_id = event.operation_id,
            actor_id = event.actor_id,
            organization_id = event.organization_id,
            command_type = event.command_type,
            outcome = event.outcome,
            latency_ms = event.latency_ms,
            message = event.message,
        ),
        "WARN" | "warn" => tracing::warn!(
            timestamp = event.timestamp,
            service = event.service,
            trace_id = event.trace_id,
            operation_id = event.operation_id,
            actor_id = event.actor_id,
            organization_id = event.organization_id,
            command_type = event.command_type,
            outcome = event.outcome,
            latency_ms = event.latency_ms,
            message = event.message,
        ),
        _ => tracing::info!(
            timestamp = event.timestamp,
            service = event.service,
            trace_id = event.trace_id,
            operation_id = event.operation_id,
            actor_id = event.actor_id,
            organization_id = event.organization_id,
            command_type = event.command_type,
            outcome = event.outcome,
            latency_ms = event.latency_ms,
            message = event.message,
        ),
    }
}

/// Recursively removes fields that can contain credentials or private key
/// material before a value reaches structured logging or audit metadata.
pub fn redact_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut output = Map::new();
            for (key, nested) in map {
                let normalized = key.to_ascii_lowercase();
                let sensitive = [
                    "password",
                    "token",
                    "secret",
                    "signature",
                    "authorization",
                    "private_key",
                    "signing_key",
                    "refresh_token",
                    "access_token",
                ]
                .iter()
                .any(|fragment| normalized.contains(fragment));
                output.insert(
                    key.clone(),
                    if sensitive {
                        Value::String("[REDACTED]".to_string())
                    } else {
                        redact_value(nested)
                    },
                );
            }
            Value::Object(output)
        }
        Value::Array(items) => Value::Array(items.iter().map(redact_value).collect()),
        other => other.clone(),
    }
}

#[derive(Default)]
struct JsonVisitor {
    fields: Map<String, Value>,
}

impl Visit for JsonVisitor {
    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), Value::Bool(value));
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields
            .insert(field.name().to_string(), Value::from(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        self.fields
            .insert(field.name().to_string(), Value::from(value));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        self.fields
            .insert(field.name().to_string(), Value::from(value));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields
            .insert(field.name().to_string(), Value::String(value.to_string()));
    }

    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.insert(
            field.name().to_string(),
            Value::String(format!("{value:?}").trim_matches('"').to_string()),
        );
    }
}

fn take_string(fields: &mut Map<String, Value>, name: &str) -> String {
    fields
        .remove(name)
        .and_then(|value| match value {
            Value::String(value) => Some(value),
            Value::Null => Some(String::new()),
            other => Some(other.to_string()),
        })
        .unwrap_or_default()
}

fn take_u64(fields: &mut Map<String, Value>, name: &str) -> u64 {
    fields
        .remove(name)
        .and_then(|value| value.as_u64())
        .unwrap_or_default()
}

/// A tracing layer that guarantees every emitted line is one JSON object with
/// the complete R3 schema. Fields not supplied by an event are emitted as
/// empty strings/zero rather than omitted. Extra fields are recursively
/// redacted and retained under `details`.
#[derive(Clone, Debug)]
pub struct CanonicalJsonLayer {
    service: String,
}

impl CanonicalJsonLayer {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }
}

impl<S> Layer<S> for CanonicalJsonLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        let mut visitor = JsonVisitor::default();
        event.record(&mut visitor);
        let mut fields = visitor.fields;
        let event_service = take_string(&mut fields, "service");
        let explicit_trace_id = take_string(&mut fields, "trace_id");
        let telemetry_context = tracing::Span::current().context();
        let telemetry_span = telemetry_context.span();
        let span_context = telemetry_span.span_context();
        let trace_id = if explicit_trace_id.is_empty() && span_context.is_valid() {
            span_context.trace_id().to_string()
        } else {
            explicit_trace_id
        };
        let record = json!({
            "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            "level": event.metadata().level().as_str(),
            "service": if event_service.is_empty() { self.service.clone() } else { event_service },
            "trace_id": trace_id,
            "operation_id": take_string(&mut fields, "operation_id"),
            "actor_id": take_string(&mut fields, "actor_id"),
            "organization_id": take_string(&mut fields, "organization_id"),
            "command_type": take_string(&mut fields, "command_type"),
            "outcome": take_string(&mut fields, "outcome"),
            "latency_ms": take_u64(&mut fields, "latency_ms"),
            "message": take_string(&mut fields, "message"),
            "target": event.metadata().target(),
            "details": redact_value(&Value::Object(fields)),
        });
        if let Ok(encoded) = serde_json::to_string(&record) {
            let _ = writeln!(std::io::stdout().lock(), "{encoded}");
        }
    }
}
