# ONYX diagnostic event contract

Schema version: 1.0. This contract covers Stage 1 Admin Shell login diagnostics, Stage 1 Cloudflare Worker login-boundary events, and Stage 2 Cloudflare Worker non-login HTTP failures.

## Required semantics

- schema_version: explicit compatibility version.
- timestamp: UTC RFC 3339 timestamp from the emitting runtime; clock skew between client and server remains possible.
- severity: severity of the observed failure, not a measure of security impact.
- component and operation: bounded component label and operation family, not raw URLs or arbitrary handler/user strings.
- stage: the known reporting/processing stage. A boundary status does not establish the failing internal dependency.
- correlation_id: validated identifier matching [A-Za-z0-9._:-]{1,80}. It is a grouping aid only, never an authorization credential.
- outcome: success, failure, degraded, or unknown.
- error_code: stable machine classification.
- cause_confidence: confirmed, suspected, or unknown. Use unknown until independent evidence proves a cause.
- evidence_source: what directly supported the classification, not an inference. Worker CORS decisions use cors_policy; a code mapped from the response uses server_code; generic HTTP classifications use http_status.
- retryability: yes, no, or unknown. Unknown is preferred over inventing a retry policy.
- remediation_ref: stable documentation reference. This is advice, not proof that the suggested cause occurred.
- context: explicitly allowlisted scalar metadata only. The JSON Schema rejects additional context keys.

## Compatibility

The Admin Shell login event keeps legacy event, request_id, code, http_status, transport_code, server_code, duration_ms and retryable aliases. The Stage 1 Worker login event keeps event, request_id, method, status, origin, origin_allowed, elapsed_ms, cause_type and diagnostic_code where those fields existed. Its old fine-grained stage, outcome and server code are retained under legacy_stage, legacy_outcome and legacy_error_code, while stage/outcome/error_code use canonical meanings. The Stage 2 Worker event keeps event, request_id, route_group, method, status, diagnostic_code, elapsed_ms, origin_allowed and cause_type. For both, correlation_id mirrors the validated request ID. New consumers must use schema_version + error_code + correlation_id and treat legacy aliases as deprecated but present for Stage 1/2 query compatibility. Removing aliases requires a separately reviewed versioned change.

The Worker reports stage=worker for generic non-login HTTP failures. A returned status and status-based code do not confirm the internal root cause; cause_confidence remains unknown. Severity/retryability are conservative status mappings and are not a prediction of service recovery.

## Runtime mapping

- Admin Shell (browser/WebView): component=admin-shell, operation=auth.login, stage from the login classifier, error_code from the stable login registry. Correlation is the request ID created by the login workflow or a local safe ID if no trusted ID is supplied. A local ID generated after a request cannot retroactively correlate a request that was sent without that ID.
- Cloudflare Worker: component=cloudflare-worker, operation is HTTP method + bounded route group, stage=worker for Stage 2's request-boundary failures, error_code is the API status classification, evidence_source=http_status. Request ID is accepted only when it matches the contract pattern; otherwise the Worker generates a UUID.
- Axum/Rust observability: existing CanonicalJsonLayer emits its R3 shape (timestamp, level, service, trace_id, operation_id, correlation_id, error_code, outcome, latency_ms, message and details). Those events are not yet validated/emitted as schema 1.0 events. Map level to severity, service to component, command_type/operation_id to operation, trace_id or correlation_id to correlation context, error_code to error_code, and latency_ms to context.duration_ms only in a downstream reader. Do not pretend that mapping was executed by this change.
- Other clients: use the shared field semantics when instrumented; they are not automatically covered by the Admin Shell implementation.

## Privacy rules

Never emit passwords, password hashes, access/refresh tokens, cookies, Authorization values, private request/response payloads, raw exception messages or stack traces, arbitrary URLs, query strings, raw user/resource/organization identifiers, or secret values. Do not add arbitrary body snapshots for correlation. Use the route_group and allowlisted context. Correlation IDs are identifiers, not access controls. Existing Rust redaction should be treated as a separate implementation with its own tests and review; this schema does not certify those details as safe.

## Operational limitations

The contract defines event shape only. It does not create a new telemetry service, endpoint, dashboard, retention guarantee or access policy. Consumers must use the log destination and retention actually configured for the emitting runtime.
