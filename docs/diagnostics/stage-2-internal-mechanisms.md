# Stage 2 — Internal Mechanism Diagnostics

Repository: `SMozaff/Onyx-Framework`

Baseline inspected: Stage 1 merge commit `28770927087783e930ecaeafbbebc4fa4e9c1d4d` (PR #280). The Stage 1 login registry, Admin Shell integration and Worker login boundary are present on `main`. CI, Cloudflare Worker CI, Security and All-Father credential smoke all reported success for that commit before Stage 2 work began.

## Scope and contract

Stage 1 defines a client-side login registry in `crates/bins/admin-shell/ui/src/auth/loginDiagnostics.ts`, with evidence requirements, stable codes, stage/severity, safe messages, retry guidance, remediation and observability location. The Worker login boundary in `deploy/cloudflare/edge-gateway/src/index.ts` already emits a sanitized login event and propagates `X-ONYX-Request-ID`.

That registry is intentionally login-specific and browser-dependent. It is not imported into the Worker or Rust server because doing so would cross runtime/language boundaries. Stage 2 reuses the compatible wire/logging conventions—stable uppercase diagnostic code, category/retryability on existing API errors, request correlation, structured JSON, and allowlisted context—without forcing identical response shapes onto unrelated components.

## Verified architecture and diagnostic coverage map

| Component | Source evidence and responsibility | Dependencies / boundaries | Existing failure evidence and tests | Stage 2 result / remaining gap |
| --- | --- | --- | --- | --- |
| Cloudflare Worker HTTP entry and router | `deploy/cloudflare/edge-gateway/src/index.ts`; dispatches auth, query, command, user/admin, All-Father organization/admin, profiles, todo aggregates, files, push, relay and event routes | Cloudflare Worker request boundary; D1; Durable Object namespaces for event/relay; configured CORS origins; optional HF/S3 configuration | `test/auth.test.ts`, `test/security.test.ts`, route-completeness tests; route handlers return stable error fields inconsistently depending on route | Added non-login request ID propagation, failure-only structured events, bounded route-family labels, status-based stable classifications, and correlation metadata on JSON errors. No request/response bodies, raw URL, headers, exception text or stack traces are logged. |
| Login path (Stage 1) | `authLogin`, `instrumentLoginRequest`, and Admin Shell `loginDiagnostics.ts` | Worker + D1 account/organization state; Admin Shell Axios client | Login tests cover generic credential denial, request IDs, malformed requests, CORS preflight and safe logs | Preserved; remains on its existing Stage 1 instrumentation path to avoid duplicate or changed login behavior. |
| Worker readiness / D1 schema | `/health`, `/ready`, `missingAuthSchema` in `index.ts` | D1 schema and availability | `/ready` returns degraded status when schema is incomplete or a D1 exception occurs | Non-login wrapper now emits a bounded service-unavailable event for 503 and propagates the request ID. Response continues to expose only degraded status/schema summary. |
| Worker authorization and tenant/resource boundaries | `src/authorization.ts`, `src/resource-scope.ts`, `src/mobile-capability.ts`, `src/command-semantics.ts`; policy derives from verified session and D1-scoped resource data | Verified JWT claims, D1, role/resource/lifecycle rules | `test/security.test.ts` includes cross-role, origin and resource-scope denials | Diagnostics classify HTTP 401/403 without logging identity, organization, token, payload or resource IDs. Authorization policy was not changed. |
| Worker WebSocket / Durable Object routes | `/api/events`, `/api/relay/*`, `src/event-hub.ts` | Durable Object namespaces and verified role/session context | Worker security tests cover role forwarding and origin rejection | HTTP error responses receive correlation; WebSocket 101 upgrade responses are intentionally not reconstructed because doing so can break platform-managed WebSocket state. Internal Durable Object message-level correlation remains a gap. |
| Axum API server and routes | `crates/bins/api-server/src/main.rs`, `src/routes/mod.rs`, route modules under `src/routes/` | PostgreSQL or SQLite primary pool; optional separate governance DB; security/application ports; blob-store adapter | `ApiErrorBody` already defines `code`, `category`, `retryability`, `safe_details`, and `correlation_id`; router has explicit auth middleware and configured CORS | No Rust route instrumentation was changed in this stage. The API server is a distinct backend from the Cloudflare Worker; request-boundary correlation and uniform failure events need a separately tested middleware change. |
| Rust observability adapter | `crates/infrastructure/observability-adapter/src/{logging,tracing,metrics,audit}.rs` | `tracing`, canonical JSON layer, optional OTLP/gRPC exporter, Prometheus metrics, hash-chain audit writer | `CanonicalJsonLayer` standardizes log fields; `redact_value` recursively redacts selected sensitive keys; metrics include request counts/latency and observability export errors | Existing redaction is useful but caller-provided free-form messages still require discipline. Stage 2 does not alter the shared Rust log schema or audit serialization. |
| Persistence and blob storage | `crates/infrastructure/persistence-postgres`, `persistence-sqlite`; `crates/bins/api-server/src/blob_storage.rs`; Worker D1 queries in `edge-gateway/src/index.ts` | PostgreSQL/SQLite/SQLx; local or Hugging Face S3-compatible blob storage; Cloudflare D1 | Startup checks and existing adapter tests; Worker `/ready` probes schema | Worker request-boundary failures are now classifiable. Database query internals and storage-provider exceptions are not exposed to clients. Provider-specific telemetry is still incomplete. |
| Background jobs and messaging | `crates/infrastructure/background-jobs/src/lib.rs` exports PostgreSQL/SQLite queue adapters; `crates/infrastructure/messaging-adapter` is present in workspace and API dependencies | Queue adapters and application ports; exact scheduling/worker lifecycle depends on concrete callers | Queue adapter tests and service-level coverage must be checked per implementation before assuming runtime deployment | Not instrumented in this stage: no evidence yet that one job runner is the single active runtime boundary for the demo. Instrument concrete queue consume/ack/retry boundaries after mapping their callers. |
| Cross-platform client error mapping | Admin Shell `src/utils/errorHandler.ts`; Web UI `src/utils/errorHandler.ts`; Mobile PWA `src/utils/errorHandler.ts`; Desktop Shell `src/utils/userFacingError.ts` | Axios, each app's API client, client-local types | UI unit tests exist; Admin Shell Stage 1 has a separate login registry. Generic no-response errors map to a broad network message in the inspected handlers | No cross-client rewrite in this stage. The generic error handlers differ by app and still collapse opaque no-response cases; avoid claiming a network cause without transport evidence. |
| Localization | `shared/i18n`, generated app dictionaries, `scripts/sync-i18n.mjs` | Shared translation catalog and per-client generated dictionaries | Existing i18n CI and verification scripts | No localization changes. Current new diagnostic codes are internal log classifications; public user messages remain the existing route-specific messages. |

## Stage 2 implementation

### Stable internal failure classification

The Worker module `deploy/cloudflare/edge-gateway/src/diagnostics.ts` defines stable, status-based classifications:

- `API_REQUEST_INVALID`
- `API_AUTHENTICATION_REQUIRED`
- `API_AUTHORIZATION_DENIED`
- `API_NOT_FOUND`
- `API_METHOD_NOT_ALLOWED`
- `API_CONFLICT`
- `API_RATE_LIMITED`
- `API_FEATURE_UNAVAILABLE`
- `API_SERVICE_UNAVAILABLE`
- `API_INTERNAL_FAILURE`
- `API_REQUEST_REJECTED`

These are internal diagnostic classifications, not a replacement for existing public route-specific `error` codes. They are assigned from HTTP status rather than inferred from exception text or request content. Successful responses do not produce failure events.

### Request correlation and event schema

For non-login HTTP requests, the Worker now:
1. Validates an incoming `X-ONYX-Request-ID` against `[A-Za-z0-9._:-]{1,80}`, generating a new ID when absent/invalid.
2. Forwards that ID in the request and response header.
3. Adds `correlation_id` to JSON error bodies when no domain-level correlation ID already exists; otherwise adds a separate `request_id` field so an existing domain correlation ID is preserved.
4. Emits `onyx.api.failure` only for HTTP status 400 or greater, with request ID, bounded route group, allowlisted method, status, stable diagnostic code, bounded elapsed time and a boolean origin decision.
5. Avoids logging request bodies, response bodies, raw paths/IDs, origins, credentials, tokens, cookies, headers, exception messages and stack traces.

The existing login path retains its Stage 1 event names and behavior. A WebSocket 101 response is not reconstructed; therefore its response cannot be decorated by this HTTP wrapper.

## Security and compatibility review

- Authentication, authorization, tenant isolation, account state, organization state and All-Father policy code were not modified.
- CORS allowlists were not changed. A disallowed origin still does not receive an allow-origin response header.
- Invalid incoming request IDs are replaced; untrusted values cannot inject new log fields because they are never interpolated into the event.
- Logs contain only a fixed event name, bounded route family, fixed classification, integer status/timing, method from a fixed allowlist, and a nullable boolean origin decision.
- Internal error responses remain generic. The wrapper adds correlation metadata but does not include exception details.
- The Worker dispatch catch no longer writes raw exception objects with `console.error(error)`; its generic 500 response is classified by the request boundary instead.
- Existing response fields are preserved. Existing domain `correlation_id` values are not overwritten.
- No new dependency, diagnostic endpoint, telemetry vendor, or permission bypass was added.

## Regression tests

New file: `deploy/cloudflare/edge-gateway/test/diagnostics.test.ts`.

It tests stable status classification, route-group privacy, request-ID validation, event schema bounds, response/header correlation, no CORS widening for disallowed origins, and a simulated D1 readiness failure whose private exception text must not appear in logs.

The pre-existing Worker auth/security tests remain the compatibility suite. The full Worker test suite and defined Worker dry-run/build check are required CI gates. No local test is claimed unless actually executed.

## Intentionally uninstrumented components

- **Axum API route middleware:** existing API error envelopes and canonical Rust logging are present, but implementing global correlation requires a dedicated middleware design and tests against Axum response/body behavior. Doing it alongside the Worker boundary would widen this patch without local Rust execution.
- **Rust persistence and blob adapters:** instrument at repository/provider boundaries only after identifying the error ownership and retry policy for each adapter. Do not log raw SQLx/provider error strings by default.
- **Background job consume/retry lifecycle:** the workspace contains queue adapters, but this audit has not established which runner is active in the demo deployment. Do not instrument a presumed worker lifecycle.
- **Durable Object internals:** HTTP correlation is added at the Worker boundary, but event/relay message-level context propagation needs separate tests through the actual Durable Object protocols.
- **Generic UI errors outside login:** the clients have different error types and behavior. Stage 1 login diagnostics should not be force-fit to every screen without mapping each API contract and its localized messages.
- **Localization catalogs:** no public copy changed, so no catalog edits were needed.

## Remaining gaps and recommended Stage 3

1. Add a separately scoped Axum request-ID middleware and structured error event tests; preserve existing `ApiErrorBody` and route authorization.
2. Trace concrete queue producer/consumer boundaries, including retry/dead-letter behavior, only after confirming active runtime composition.
3. Add provider-specific persistence/blob diagnostics that preserve source errors internally while returning safe public errors.
4. Propagate correlation through Durable Object event/relay messages where the protocol permits it.
5. Harmonize non-login client error mapping across Admin Shell, Web UI, Desktop Shell and Mobile PWA, with localization fallback tests.
6. Capture a real request ID from a packaged Tauri login and compare it to live Worker logs before drawing conclusions about live-origin or deployment issues.

## Verification record

Baseline: `28770927087783e930ecaeafbbebc4fa4e9c1d4d`.

Stage 2 CI verification must be recorded against the final commit SHA after the branch changes are complete. The architecture map above records repository evidence, not a claim that every listed runtime component is deployed or active.
