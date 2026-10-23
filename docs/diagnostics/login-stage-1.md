# ONYX Stage 1 — Login diagnostics and error intelligence

**Status:** merged into main via [PR #280](https://github.com/SMozaff/Onyx-Framework/pull/280) on 2026-10-22. Source presence is confirmed by the current repository; live deployment of every diagnostic change is not established by merge status alone.  
**Repository baseline:** `main` at `0e83e8713cc8c84a169dbf6aef129f1255867cc6` when the branch was created.  
**Scope:** Admin Shell login request, Cloudflare Worker login route, request correlation, sanitized diagnostic events, stable error registry, and focused tests.

## Evidence labels

- **Confirmed in source** means the behavior is present in the repository revision inspected.
- **Confirmed by live read-only API** means current Cloudflare metadata returned it during this audit. It does not prove that every request reaches that deployment.
- **Hypothesis** means a possible explanation that still needs a correlated observation.
- **Unknown** means the required evidence was not available.

## Login request trace

1. **Login form:** `crates/bins/admin-shell/ui/src/pages/Login.tsx` handles submission. It validates the configured endpoint, creates a request ID, calls the existing credential helper, stores the returned session, and navigates to the current post-login route. Failure UI uses registry-defined safe messages; raw exception messages are not shown.
2. **Endpoint configuration:** `crates/bins/admin-shell/ui/src/utils/serverAddress.ts` fixes the API base at `https://onyx-framework.soheil-mozaffari.workers.dev`. The request URL in `crates/bins/admin-shell/ui/src/auth/credentials.ts` is `/api/auth/login`.
3. **HTTP client:** `crates/bins/admin-shell/ui/src/api/client.ts` is Axios-based, sets a 30,000 ms timeout, assigns the base URL, and may attach an existing access-token header. The new request ID is sent as `X-ONYX-Request-ID`. A cross-origin JSON POST already uses a CORS preflight; this custom header is also listed in the preflight allow-headers policy.
4. **Tauri/WebView:** `crates/bins/admin-shell/tauri.conf.json` sets development URL `http://localhost:5174`; packaged origin was not observed in this repository audit. Do not infer packaged origin from the development URL.
5. **Cloudflare Worker:** `deploy/cloudflare/edge-gateway/src/index.ts` dispatches `POST /api/auth/login` to `authLogin`; login does not invoke the Rust Axum API in this source. The Worker enforces an exact configured origin allowlist in CORS.
6. **Credential checks:** the Worker queries D1 for an active user joined to an active organization; it checks the explicit role, reserved/canonical All-Father identity, and Argon2id password hash. Its public response deliberately collapses unknown user, wrong password, and several account-state failures to `INVALID_CREDENTIALS` / 401. The new instrumentation preserves that behavior.
7. **Session issuance:** the Worker creates the existing access/refresh token pair using the configured signing secret. New internal errors are returned as safe machine codes and logged without exception messages or request/session contents.
8. **Client validation and persistence:** `credentials.ts` validates the existing token-pair/user response contract. `stores/authStore.ts` persists the existing session via `sessionStorage`. The login page then navigates using the existing client router. Diagnostics distinguish invalid response, session persistence exception, and navigation exception.

## Stable error registry

Machine-readable definitions are maintained in `crates/bins/admin-shell/ui/src/auth/loginDiagnostics.ts`. Each definition contains a stable code, category, stage, severity, safe message, internal diagnostic, retry guidance, evidence requirement, remediation, and expected observability location.

| Code | Evidence needed before assignment | Observability |
| --- | --- | --- |
| `LOGIN_CONFIG_MISSING`, `LOGIN_CONFIG_INVALID` | Endpoint absent or fails exact HTTPS endpoint validation before dispatch | Client |
| `LOGIN_DNS_FAILURE` | Explicit native transport DNS code | Cross-component |
| `LOGIN_CONNECTION_FAILURE` | Explicit connection-refused/reset/unreachable code | Cross-component |
| `LOGIN_TLS_FAILURE` | Explicit TLS/certificate error code | Cross-component |
| `LOGIN_TIMEOUT` | Explicit Axios timeout code | Cross-component |
| `LOGIN_RESPONSE_UNAVAILABLE` | Axios error has no response and no more specific transport code | Cross-component |
| `LOGIN_REQUEST_CANCELLED` | Explicit client cancellation code | Client |
| `LOGIN_CORS_ORIGIN_REJECTED` | Readable origin-denial machine code or matching Worker evidence | Server |
| `LOGIN_HTTP_ERROR` | Readable HTTP error without a more specific known code | Cross-component |
| `LOGIN_ROUTE_NOT_FOUND`, `LOGIN_METHOD_NOT_ALLOWED` | Readable 404/405 or matching machine code | Cross-component |
| `LOGIN_REQUEST_INVALID`, `LOGIN_CLIENT_TYPE_INVALID` | Explicit request-validation/client-type machine code | Cross-component |
| `LOGIN_AUTH_REJECTED` | `INVALID_CREDENTIALS` response, not the UI message | Server |
| `LOGIN_AUTHORIZATION_DENIED` | Explicit authorization code/status | Server |
| `LOGIN_ACCOUNT_OR_ORGANIZATION_UNAVAILABLE` | Authorized server-side account/org evidence; never distinguish via public login response | Server |
| `LOGIN_RATE_LIMITED` | HTTP 429 or explicit rate-limit code | Server |
| `LOGIN_BACKEND_FAILURE` | Explicit safe backend code or correlated server diagnostic | Cross-component |
| `LOGIN_SESSION_CREATION_FAILED` | Explicit session-creation code or correlated Worker stage | Server |
| `LOGIN_SESSION_PERSISTENCE_FAILED` | Client storage exception after response validation | Client |
| `LOGIN_UNEXPECTED_RESPONSE` | Existing session validator rejects the successful response shape | Cross-component |
| `LOGIN_CLIENT_PARSING_FAILED` | Explicit client response-parsing exception | Client |
| `LOGIN_NAVIGATION_FAILED` | Navigation throws after session persistence | Client |
| `LOGIN_UNKNOWN` | No more-specific, evidence-backed mapping applies | Cross-component |

A client-side error with no accessible HTTP response is **not proof of CORS**. In webviews, CORS denial, DNS, TLS, network failure, offline state, and other transport errors can be opaque. The client labels this `LOGIN_RESPONSE_UNAVAILABLE` unless a precise transport error exists. Worker-side preflight logs can show whether the observed origin was in the live allowlist.

## Diagnostic event and privacy contract

Client event name: `onyx:login-diagnostic`; console event prefix: `[ONYX_LOGIN_DIAGNOSTIC]`. Each login failure displayed by the Admin Shell includes its stable code and a safe reference ID so a user can report the exact attempt without opening Developer Tools. Server events: `onyx.login.http` and `onyx.login.failure`.

Allowlisted event attributes: request ID, stable code, stage, severity, HTTP status, transport code, server code, duration, retryability, and a truncated origin needed for CORS investigation. Worker request IDs must match `[A-Za-z0-9._:-]{1,80}`; invalid incoming values are replaced with a generated ID. JSON error responses include `code` and `correlation_id`; responses expose `X-ONYX-Request-ID` only to an allowed origin.

Never log passwords, password hashes, access/refresh tokens, cookies, authorization headers, arbitrary request/response bodies, secret environment-variable values, or raw exception messages. Worker unexpected-error telemetry records only a cause type and the internal stage/code.

**Preflight limitation:** browsers generally do not send the custom request-ID header value on the OPTIONS preflight; the preflight contains the requested header *names*. Therefore a rejected preflight may have a server-generated ID that is not visible to the client. Diagnose that case using its safe preflight log, exact origin, time window and method. The client request ID reliably correlates the POST if the POST is sent.

## CORS #276 and present live evidence

PR [#276](https://github.com/SMozaff/Onyx-Framework/pull/276) historically added `http://localhost:5174` to the production deployment workflow's CORS allowlist. The live Cloudflare settings were independently read during this stage. The current live `ONYX_CORS_ORIGINS` binding includes `http://localhost:5174`, and does not include `http://127.0.0.1:5174`, `tauri://localhost`, or `http://tauri.localhost`. The actual Origin emitted by the user's packaged Tauri WebView has not been captured. These facts do **not** prove that CORS is or is not the current login failure.

**Compatibility/rollout:** this client sends the diagnostic ID in the custom `X-ONYX-Request-ID` header, which must appear in the deployed Worker’s `Access-Control-Allow-Headers` response. Release the Worker and Admin Shell as a coordinated pair. Until a Worker version allowing this header is active, a feature-branch Admin Shell can fail preflight even when its origin is allowed. Check the live preflight response before using this build against the live endpoint. No deployment is performed by this PR.

The live Worker reported deployment `319c63f9-f3d2-4881-8a59-2f2997bd1059` with version `7e11d3df-60ed-4b0f-b0c0-50dc4e808571` receiving 100% traffic at the time checked. The deployment occurred before the currently inspected `main` merge commit; source-to-live-version parity was not established. No live login attempt was made and no real credentials were used.

## Files changed

- `crates/bins/admin-shell/ui/src/auth/loginDiagnostics.ts` — typed registry, evidence-gated classifier, config validation, safe request IDs and client events.
- `crates/bins/admin-shell/ui/src/auth/credentials.ts` — adds request ID propagation without changing request body or authentication contract.
- `crates/bins/admin-shell/ui/src/pages/Login.tsx` — integrates diagnostics and safe stage-specific user messages while retaining current login/session/navigation flow.
- `crates/bins/admin-shell/ui/tests/loginDiagnostics.test.ts` — error mapping, evidence threshold, config and privacy tests.
- `crates/bins/admin-shell/ui/tests/onyxCredentials.test.ts` — request ID header contract test.
- `deploy/cloudflare/edge-gateway/src/index.ts` — Worker request ID, safe structured login/preflight telemetry, validation/route codes, safe infrastructure errors, and response correlation.
- `deploy/cloudflare/edge-gateway/test/auth.test.ts` — Worker tests for error envelopes, request IDs, preflight, auth route/method errors, and secret-safe logging.
- This document — evidence trace, registry and deployment limitations.

## Verification and unresolved work

The tests use mocked D1/network components where applicable; they do not demonstrate a real user login. CI for the final feature-branch SHA is the verification authority for type-checking/build/test outcomes. Until its result is recorded, tests must be described as added but not yet verified.

Unresolved items:
1. The packaged Tauri WebView's actual Origin and a matching POST/preflight observation are still required to accept or reject the CORS hypothesis.
2. The worker deployment's source commit SHA and deployment configuration parity are not established.
3. No live login attempt was made; secret values were not read, displayed, or tested.
4. A request ID may not cross the browser's failed-preflight boundary; server-side origin/time evidence is required in that case.
5. The current Worker login contract deliberately does not expose whether a user exists or whether the account/organization is inactive. Keep this privacy-preserving behavior.
