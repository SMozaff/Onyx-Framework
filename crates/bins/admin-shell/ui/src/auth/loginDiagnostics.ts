import axios from "axios";
import { isSecureEnoughForProduction } from "../utils/serverAddress";

export type LoginSeverity = "info" | "warning" | "error" | "critical";
export type LoginStage =
  | "configuration" | "transport" | "cors" | "routing" | "request_validation"
  | "authentication" | "authorization" | "backend" | "session" | "client"
  | "navigation" | "unknown";
export type LoginObservability = "client" | "server" | "cross_component";
export type LoginErrorCode =
  | "LOGIN_CONFIG_MISSING" | "LOGIN_CONFIG_INVALID"
  | "LOGIN_DNS_FAILURE" | "LOGIN_CONNECTION_FAILURE" | "LOGIN_TLS_FAILURE"
  | "LOGIN_TIMEOUT" | "LOGIN_RESPONSE_UNAVAILABLE" | "LOGIN_REQUEST_CANCELLED"
  | "LOGIN_CORS_ORIGIN_REJECTED" | "LOGIN_HTTP_ERROR" | "LOGIN_ROUTE_NOT_FOUND"
  | "LOGIN_METHOD_NOT_ALLOWED" | "LOGIN_REQUEST_INVALID" | "LOGIN_CLIENT_TYPE_INVALID"
  | "LOGIN_AUTH_REJECTED" | "LOGIN_AUTHORIZATION_DENIED"
  | "LOGIN_ACCOUNT_OR_ORGANIZATION_UNAVAILABLE" | "LOGIN_RATE_LIMITED"
  | "LOGIN_BACKEND_FAILURE" | "LOGIN_SESSION_CREATION_FAILED"
  | "LOGIN_SESSION_PERSISTENCE_FAILED" | "LOGIN_UNEXPECTED_RESPONSE"
  | "LOGIN_CLIENT_PARSING_FAILED" | "LOGIN_NAVIGATION_FAILED" | "LOGIN_UNKNOWN";

export interface LoginErrorDefinition {
  category: string;
  stage: LoginStage;
  severity: LoginSeverity;
  userMessage: string;
  internalDiagnostic: string;
  retryable: boolean;
  retryGuidance: string;
  evidenceRequired: string;
  remediation: string;
  observability: LoginObservability;
}

export const LOGIN_ERROR_REGISTRY: Readonly<Record<LoginErrorCode, LoginErrorDefinition>> = {
  LOGIN_CONFIG_MISSING: { category: "configuration", stage: "configuration", severity: "critical", userMessage: "The login service is not configured. Contact an administrator.", internalDiagnostic: "The configured API endpoint is empty or absent.", retryable: false, retryGuidance: "Do not retry until configuration is corrected.", evidenceRequired: "Empty or missing configured endpoint before sending the request.", remediation: "Restore the documented HTTPS Worker endpoint; do not read secrets to diagnose this.", observability: "client" },
  LOGIN_CONFIG_INVALID: { category: "configuration", stage: "configuration", severity: "critical", userMessage: "The login service configuration is invalid. Contact an administrator.", internalDiagnostic: "The configured endpoint does not match the supported secure endpoint.", retryable: false, retryGuidance: "Do not retry until configuration is corrected.", evidenceRequired: "Endpoint validation fails before HTTP dispatch.", remediation: "Correct the endpoint configuration and verify it against the supported API address.", observability: "client" },
  LOGIN_DNS_FAILURE: { category: "transport", stage: "transport", severity: "error", userMessage: "The login service could not be resolved. Check connectivity and try again.", internalDiagnostic: "Name resolution failure explicitly reported by the native transport; opaque browser errors do not qualify.", retryable: true, retryGuidance: "Retry after DNS resolution is restored.", evidenceRequired: "Native/network telemetry explicitly reports a DNS resolution error for the API hostname.", remediation: "Verify resolver output and hostname without changing credentials.", observability: "cross_component" },
  LOGIN_CONNECTION_FAILURE: { category: "transport", stage: "transport", severity: "error", userMessage: "The login service could not be reached. Check connectivity and try again.", internalDiagnostic: "Connection failure explicitly reported by the network adapter; opaque browser errors do not qualify.", retryable: true, retryGuidance: "Retry after connectivity is restored.", evidenceRequired: "Transport error code explicitly indicates connection refusal/reset/unreachable host.", remediation: "Verify the route, endpoint reachability, and Cloudflare edge availability.", observability: "cross_component" },
  LOGIN_TLS_FAILURE: { category: "transport", stage: "transport", severity: "critical", userMessage: "A secure connection to the login service could not be established.", internalDiagnostic: "TLS certificate or handshake failure explicitly reported by the native transport.", retryable: false, retryGuidance: "Do not bypass certificate validation; retry after TLS is corrected.", evidenceRequired: "Native transport explicitly reports a TLS/certificate/handshake error.", remediation: "Validate certificate chain, hostname, validity, and supported TLS settings.", observability: "cross_component" },
  LOGIN_TIMEOUT: { category: "transport", stage: "transport", severity: "warning", userMessage: "The login request timed out. Try again.", internalDiagnostic: "HTTP client explicitly reported its configured request timeout.", retryable: true, retryGuidance: "Retry once after checking service health; avoid rapid repeated attempts.", evidenceRequired: "Axios reports ECONNABORTED or ETIMEDOUT for the login request.", remediation: "Correlate request ID with Worker timing and determine whether the request reached the edge.", observability: "cross_component" },
  LOGIN_RESPONSE_UNAVAILABLE: { category: "transport", stage: "transport", severity: "error", userMessage: "The login service did not return a response. Check your connection and try again.", internalDiagnostic: "Axios exposed no HTTP response. This is intentionally opaque and may be CORS, DNS, TLS, connection, offline state, or Tauri/WebView transport.", retryable: true, retryGuidance: "Retry after checking connectivity; use the request ID and server logs before attributing a cause.", evidenceRequired: "Axios error with no response and no explicit timeout/DNS/TLS/connection code.", remediation: "Correlate the client request ID with Worker preflight/request logs; do not label this CORS without server evidence.", observability: "cross_component" },
  LOGIN_REQUEST_CANCELLED: { category: "transport", stage: "transport", severity: "info", userMessage: "The login request was cancelled. Try again when ready.", internalDiagnostic: "The client explicitly cancelled the request.", retryable: true, retryGuidance: "Retry only when the user initiates the login again.", evidenceRequired: "Axios reports ERR_CANCELED or an equivalent explicit cancellation code.", remediation: "Check caller cancellation/lifecycle handling.", observability: "client" },
  LOGIN_CORS_ORIGIN_REJECTED: { category: "cors", stage: "cors", severity: "error", userMessage: "This app origin is not permitted to sign in. Contact an administrator.", internalDiagnostic: "The Worker has evidence that the request origin is outside the exact CORS allowlist. A no-response client error alone is insufficient.", retryable: false, retryGuidance: "Do not retry until the exact expected origin is allowed.", evidenceRequired: "Worker event for the matching request/preflight records origin_allowed=false, or a readable ORIGIN_NOT_ALLOWED response.", remediation: "Compare the observed Origin with the live ONYX_CORS_ORIGINS allowlist; add only a verified exact origin through the approved workflow.", observability: "server" },
  LOGIN_HTTP_ERROR: { category: "http", stage: "backend", severity: "error", userMessage: "The login request was rejected by the service. Try again or contact an administrator.", internalDiagnostic: "The server returned an HTTP error that does not match a more specific, evidence-backed registry entry.", retryable: false, retryGuidance: "Use HTTP status, safe machine code, and request ID before retrying.", evidenceRequired: "A readable HTTP response without a more specific known machine code.", remediation: "Inspect the safe error envelope and correlated server event; never log request/response bodies.", observability: "cross_component" },
  LOGIN_ROUTE_NOT_FOUND: { category: "routing", stage: "routing", severity: "error", userMessage: "The login endpoint is unavailable at this address. Contact an administrator.", internalDiagnostic: "A readable route-not-found response was returned for the configured login endpoint.", retryable: false, retryGuidance: "Do not repeatedly retry an unchanged URL.", evidenceRequired: "HTTP 404 or explicit AUTH_ROUTE_NOT_FOUND/ROUTE_NOT_FOUND response from the endpoint.", remediation: "Verify route registration and that the deployed Worker matches the intended source.", observability: "cross_component" },
  LOGIN_METHOD_NOT_ALLOWED: { category: "routing", stage: "routing", severity: "error", userMessage: "The login request method is not supported by the service.", internalDiagnostic: "The endpoint returned 405 or an explicit METHOD_NOT_ALLOWED machine code.", retryable: false, retryGuidance: "Do not retry with the same method/route combination.", evidenceRequired: "HTTP 405 or explicit METHOD_NOT_ALLOWED code.", remediation: "Use POST /api/auth/login and verify reverse proxy/Worker routing.", observability: "cross_component" },
  LOGIN_REQUEST_INVALID: { category: "validation", stage: "request_validation", severity: "warning", userMessage: "The login request could not be validated. Please try again.", internalDiagnostic: "The server explicitly reported malformed JSON or an invalid login request schema.", retryable: false, retryGuidance: "Correct request construction before retrying.", evidenceRequired: "Explicit INVALID_LOGIN_REQUEST response; HTTP status alone is insufficient.", remediation: "Verify JSON serialization and the documented username/password/client_type request contract.", observability: "cross_component" },
  LOGIN_CLIENT_TYPE_INVALID: { category: "validation", stage: "request_validation", severity: "error", userMessage: "The app sent an unsupported login configuration. Contact an administrator.", internalDiagnostic: "The Worker explicitly rejected client_type.", retryable: false, retryGuidance: "Do not retry until the fixed client contract is corrected.", evidenceRequired: "Explicit INVALID_CLIENT_TYPE response.", remediation: "Keep client_type set to the existing supported Admin Shell value; do not infer authority from it.", observability: "cross_component" },
  LOGIN_AUTH_REJECTED: { category: "authentication", stage: "authentication", severity: "warning", userMessage: "The username or password is incorrect.", internalDiagnostic: "The Worker returned INVALID_CREDENTIALS. The response intentionally does not distinguish an unknown user, wrong password, inactive account, or inactive organization.", retryable: false, retryGuidance: "Check the supplied credentials; avoid repeated attempts.", evidenceRequired: "HTTP response with machine code INVALID_CREDENTIALS, not the visual message alone.", remediation: "Verify account provisioning through approved administrative procedures; never change or expose credentials in diagnostics.", observability: "server" },
  LOGIN_AUTHORIZATION_DENIED: { category: "authorization", stage: "authorization", severity: "warning", userMessage: "This account is not authorized to sign in here. Contact an administrator.", internalDiagnostic: "The server explicitly returned an authorization denial for the login request.", retryable: false, retryGuidance: "Do not retry until the authorization configuration is reviewed.", evidenceRequired: "Readable FORBIDDEN/authorization machine code; do not infer this from generic 401.", remediation: "Review the authoritative role/account/organization state without logging personal identifiers.", observability: "server" },
  LOGIN_ACCOUNT_OR_ORGANIZATION_UNAVAILABLE: { category: "authentication", stage: "authentication", severity: "warning", userMessage: "The account could not be authenticated. Contact an administrator if this continues.", internalDiagnostic: "Admin-only server-side evidence confirms an inactive account or organization. This distinction is deliberately not exposed to unauthenticated clients.", retryable: false, retryGuidance: "Retry only after account state is corrected.", evidenceRequired: "Authorized server-side account/org state inspection; the public INVALID_CREDENTIALS response does not prove this.", remediation: "Review authoritative D1 account and organization status privately; do not emit usernames or account IDs in general logs.", observability: "server" },
  LOGIN_RATE_LIMITED: { category: "rate_limit", stage: "backend", severity: "warning", userMessage: "There have been too many login attempts. Wait before trying again.", internalDiagnostic: "The endpoint explicitly returned HTTP 429 or a recognized rate-limit code.", retryable: true, retryGuidance: "Wait for the server-provided retry window if present.", evidenceRequired: "HTTP 429 or an explicit rate-limit machine code.", remediation: "Inspect rate-limit policy and edge metrics; do not weaken authentication controls.", observability: "server" },
  LOGIN_BACKEND_FAILURE: { category: "infrastructure", stage: "backend", severity: "error", userMessage: "The login service could not complete the request. Try again later.", internalDiagnostic: "The Worker reported a failure during account lookup/verification or another backend operation; exception messages are not sent to clients.", retryable: true, retryGuidance: "Retry after checking service health and the correlated Worker event.", evidenceRequired: "Explicit LOGIN_BACKEND_FAILURE response or a correlated server error event.", remediation: "Inspect D1/Worker health and the sanitized stage/cause type; do not dump exception messages or payloads.", observability: "cross_component" },
  LOGIN_SESSION_CREATION_FAILED: { category: "session", stage: "session", severity: "error", userMessage: "The credentials were accepted, but a session could not be created. Try again later.", internalDiagnostic: "The Worker reached session issuance and failed while creating the access/refresh token pair.", retryable: true, retryGuidance: "Retry only after checking the correlated Worker error event.", evidenceRequired: "Explicit LOGIN_SESSION_CREATION_FAILED response or correlated server stage session_creation.", remediation: "Check token-signing configuration and runtime health without reading or logging secret values.", observability: "server" },
  LOGIN_SESSION_PERSISTENCE_FAILED: { category: "session", stage: "session", severity: "error", userMessage: "Sign-in succeeded, but this device could not save the session. Check app storage and try again.", internalDiagnostic: "Client sessionStorage/state persistence failed after a valid session response.", retryable: true, retryGuidance: "Retry after app storage is available; do not send credentials repeatedly if the service already accepted them.", evidenceRequired: "An exception from client session persistence after response validation.", remediation: "Check WebView sessionStorage permissions/lifecycle; never log tokens or serialized session data.", observability: "client" },
  LOGIN_UNEXPECTED_RESPONSE: { category: "response_contract", stage: "client", severity: "error", userMessage: "The login service returned an unexpected response. Contact an administrator if it continues.", internalDiagnostic: "The HTTP client returned a success response whose payload failed the existing ONYX session contract.", retryable: false, retryGuidance: "Retry once; if repeatable, inspect only the response schema and request ID.", evidenceRequired: "INVALID_AUTH_RESPONSE raised by the session shape validator.", remediation: "Compare the deployed Worker response contract with the Admin Shell session validator; never log token values.", observability: "cross_component" },
  LOGIN_CLIENT_PARSING_FAILED: { category: "client", stage: "client", severity: "error", userMessage: "The login response could not be processed. Contact an administrator if it continues.", internalDiagnostic: "Client parsing failed after the HTTP request completed.", retryable: false, retryGuidance: "Retry once; repeated errors require a contract check.", evidenceRequired: "A parsing exception at a known response-processing stage.", remediation: "Validate response shape and content type without printing response bodies.", observability: "client" },
  LOGIN_NAVIGATION_FAILED: { category: "navigation", stage: "navigation", severity: "error", userMessage: "Sign-in completed, but the app could not open the next screen. Reopen the app or contact an administrator.", internalDiagnostic: "The login session was validated and persisted but post-login navigation failed.", retryable: true, retryGuidance: "Reopen the app before submitting credentials again.", evidenceRequired: "An exception from the post-login navigation step after session persistence.", remediation: "Inspect client routing state; do not repeat authentication automatically.", observability: "client" },
  LOGIN_UNKNOWN: { category: "unknown", stage: "unknown", severity: "error", userMessage: "The login could not be completed. Try again, or contact an administrator if it continues.", internalDiagnostic: "An error did not match any evidence-backed login failure mapping.", retryable: true, retryGuidance: "Retry once; if it persists, use the request ID and sanitized diagnostics.", evidenceRequired: "No matching known machine code, HTTP status pattern, or explicit transport evidence.", remediation: "Add a mapping only after recording reproducible evidence; never expose raw exceptions.", observability: "cross_component" }
};

export interface ClassifiedLoginFailure extends LoginErrorDefinition {
  code: LoginErrorCode;
  requestId?: string;
  httpStatus?: number;
  transportCode?: string;
  serverCode?: string;
}

const KNOWN_SERVER_CODES = new Set([
  "INVALID_CREDENTIALS", "INVALID_LOGIN_REQUEST", "INVALID_CLIENT_TYPE",
  "LOGIN_BACKEND_FAILURE", "LOGIN_SESSION_CREATION_FAILED", "METHOD_NOT_ALLOWED",
  "AUTH_ROUTE_NOT_FOUND", "ORIGIN_NOT_ALLOWED", "FORBIDDEN", "UNAUTHORIZED",
  "INTERNAL_SERVER_ERROR", "RATE_LIMITED"
]);
const KNOWN_TRANSPORT_CODES = new Set([
  "ECONNABORTED", "ETIMEDOUT", "ERR_CANCELED", "ENOTFOUND", "EAI_AGAIN",
  "ECONNREFUSED", "ECONNRESET", "EHOSTUNREACH", "ENETUNREACH",
  "ERR_TLS_CERT_ALTNAME_INVALID", "CERT_HAS_EXPIRED", "UNABLE_TO_VERIFY_LEAF_SIGNATURE",
  "ERR_TLS_CERT_AUTHORITY_INVALID", "ERR_NETWORK"
]);
const SAFE_REQUEST_ID = /^[A-Za-z0-9._:-]{1,80}$/;

function safeRequestId(value: unknown): string | undefined {
  return typeof value === "string" && SAFE_REQUEST_ID.test(value) ? value : undefined;
}

function readServerCode(data: unknown): string | undefined {
  if (!data || typeof data !== "object") return undefined;
  const body = data as Record<string, unknown>;
  const value = typeof body.error === "string" ? body.error : body.code;
  return typeof value === "string" && KNOWN_SERVER_CODES.has(value) ? value : undefined;
}

export function getLoginConfigurationFailure(endpoint: unknown): LoginErrorCode | null {
  if (typeof endpoint !== "string" || endpoint.trim() === "") return "LOGIN_CONFIG_MISSING";
  return isSecureEnoughForProduction(endpoint) ? null : "LOGIN_CONFIG_INVALID";
}

export function createLoginRequestId(): string {
  try {
    if (typeof crypto !== "undefined" && typeof crypto.randomUUID === "function") return crypto.randomUUID();
  } catch {
    // WebView environments without crypto.randomUUID use the local fallback below.
  }
  return "login-" + Date.now().toString(36) + "-" + Math.random().toString(36).slice(2, 12);
}

export function loginFailureForCode(code: LoginErrorCode, requestId?: string): ClassifiedLoginFailure {
  const safeCode: LoginErrorCode = Object.prototype.hasOwnProperty.call(LOGIN_ERROR_REGISTRY, code) ? code : "LOGIN_UNKNOWN";
  return { code: safeCode, ...LOGIN_ERROR_REGISTRY[safeCode], requestId: safeRequestId(requestId) };
}

export function classifyLoginFailure(error: unknown, requestId?: string): ClassifiedLoginFailure {
  if (error instanceof SyntaxError) {
    return loginFailureForCode("LOGIN_CLIENT_PARSING_FAILED", requestId);
  }
  if (error instanceof Error && error.message === "INVALID_AUTH_RESPONSE") {
    return loginFailureForCode("LOGIN_UNEXPECTED_RESPONSE", requestId);
  }
  const candidate = error as {
    code?: unknown;
    response?: { status?: unknown; data?: unknown; headers?: unknown };
    isAxiosError?: unknown;
  } | null;
  const isAxios = axios.isAxiosError(error) || candidate?.isAxiosError === true;
  const status = typeof candidate?.response?.status === "number" ? candidate.response.status : undefined;
  const serverCode = readServerCode(candidate?.response?.data);
  const headers = candidate?.response?.headers as { get?: (name: string) => unknown } | Record<string, unknown> | undefined;
  const responseId = headers && typeof (headers as { get?: (name: string) => unknown }).get === "function"
    ? (headers as { get: (name: string) => unknown }).get("x-onyx-request-id")
    : headers && typeof (headers as Record<string, unknown>)["x-onyx-request-id"] === "string"
      ? (headers as Record<string, unknown>)["x-onyx-request-id"]
      : undefined;
  const correlation = safeRequestId(responseId) ?? safeRequestId(requestId);

  if (serverCode === "INVALID_CREDENTIALS") return { ...loginFailureForCode("LOGIN_AUTH_REJECTED", correlation), httpStatus: status, serverCode };
  if (serverCode === "INVALID_LOGIN_REQUEST") return { ...loginFailureForCode("LOGIN_REQUEST_INVALID", correlation), httpStatus: status, serverCode };
  if (serverCode === "INVALID_CLIENT_TYPE") return { ...loginFailureForCode("LOGIN_CLIENT_TYPE_INVALID", correlation), httpStatus: status, serverCode };
  if (serverCode === "ORIGIN_NOT_ALLOWED") return { ...loginFailureForCode("LOGIN_CORS_ORIGIN_REJECTED", correlation), httpStatus: status, serverCode };
  if (serverCode === "LOGIN_SESSION_CREATION_FAILED") return { ...loginFailureForCode("LOGIN_SESSION_CREATION_FAILED", correlation), httpStatus: status, serverCode };
  if (serverCode === "LOGIN_BACKEND_FAILURE" || serverCode === "INTERNAL_SERVER_ERROR") return { ...loginFailureForCode("LOGIN_BACKEND_FAILURE", correlation), httpStatus: status, serverCode };
  if (serverCode === "METHOD_NOT_ALLOWED" || status === 405) return { ...loginFailureForCode("LOGIN_METHOD_NOT_ALLOWED", correlation), httpStatus: status, serverCode };
  if (serverCode === "AUTH_ROUTE_NOT_FOUND" || status === 404) return { ...loginFailureForCode("LOGIN_ROUTE_NOT_FOUND", correlation), httpStatus: status, serverCode };
  if (serverCode === "FORBIDDEN" || status === 403) return { ...loginFailureForCode("LOGIN_AUTHORIZATION_DENIED", correlation), httpStatus: status, serverCode };
  if (serverCode === "RATE_LIMITED" || status === 429) return { ...loginFailureForCode("LOGIN_RATE_LIMITED", correlation), httpStatus: status, serverCode };
  if (isAxios && !candidate?.response) {
    const rawCode = typeof candidate?.code === "string" ? candidate.code : "";
    const code = KNOWN_TRANSPORT_CODES.has(rawCode) ? rawCode : "";
    if (code === "ECONNABORTED" || code === "ETIMEDOUT") return { ...loginFailureForCode("LOGIN_TIMEOUT", correlation), transportCode: code };
    if (code === "ERR_CANCELED") return { ...loginFailureForCode("LOGIN_REQUEST_CANCELLED", correlation), transportCode: code };
    if (code === "ENOTFOUND" || code === "EAI_AGAIN") return { ...loginFailureForCode("LOGIN_DNS_FAILURE", correlation), transportCode: code };
    if (["ECONNREFUSED", "ECONNRESET", "EHOSTUNREACH", "ENETUNREACH"].includes(code)) return { ...loginFailureForCode("LOGIN_CONNECTION_FAILURE", correlation), transportCode: code };
    if (["ERR_TLS_CERT_ALTNAME_INVALID", "CERT_HAS_EXPIRED", "UNABLE_TO_VERIFY_LEAF_SIGNATURE", "ERR_TLS_CERT_AUTHORITY_INVALID"].includes(code)) return { ...loginFailureForCode("LOGIN_TLS_FAILURE", correlation), transportCode: code };
    return { ...loginFailureForCode("LOGIN_RESPONSE_UNAVAILABLE", correlation), transportCode: code || undefined };
  }
  if (status !== undefined && status >= 500) return { ...loginFailureForCode("LOGIN_BACKEND_FAILURE", correlation), httpStatus: status, serverCode };
  if (isAxios && status !== undefined) return { ...loginFailureForCode("LOGIN_HTTP_ERROR", correlation), httpStatus: status, serverCode };
  return loginFailureForCode("LOGIN_UNKNOWN", correlation);
}

export function formatLoginFailureMessage(
  failure: Pick<ClassifiedLoginFailure, "code" | "userMessage" | "requestId">,
  fallbackRequestId?: string,
  preferredMessage?: string,
): string {
  const reference = safeRequestId(failure.requestId) ?? safeRequestId(fallbackRequestId);
  const message = typeof preferredMessage === "string" && preferredMessage.trim()
    ? preferredMessage.trim()
    : failure.userMessage;
  const code = Object.prototype.hasOwnProperty.call(LOGIN_ERROR_REGISTRY, failure.code)
    ? failure.code
    : "LOGIN_UNKNOWN";
  return `${message} (${code}${reference ? `; reference ${reference}` : ""})`;
}

export interface LoginDiagnosticEvent {
  schema_version: "1.0";
  timestamp: string;
  component: "admin-shell";
  operation: "auth.login";
  correlation_id: string;
  error_code: string;
  cause_confidence: "unknown";
  evidence_source: "http_status" | "server_code" | "transport_code" | "client_stage" | "unknown";
  retryability: "yes" | "no";
  remediation_ref: string;
  context: {
    http_status?: number;
    transport_code?: string;
    server_code?: string;
    duration_ms?: number;
  };
  // Compatibility aliases retained for the Stage 1 login event contract.
  event: "onyx.login.diagnostic";
  outcome: "success" | "failure";
  request_id?: string;
  code: string;
  stage: LoginStage;
  severity: LoginSeverity;
  http_status?: number;
  transport_code?: string;
  server_code?: string;
  duration_ms?: number;
  retryable: boolean;
}

export function recordLoginDiagnostic(input: {
  outcome: "success" | "failure";
  code: string;
  stage: LoginStage;
  severity: LoginSeverity;
  requestId?: string;
  httpStatus?: number;
  transportCode?: string;
  serverCode?: string;
  durationMs?: number;
  retryable: boolean;
}): void {
  const validStages = new Set<LoginStage>([
    "configuration", "transport", "cors", "routing", "request_validation", "authentication",
    "authorization", "backend", "session", "client", "navigation", "unknown",
  ]);
  const validSeverities = new Set<LoginSeverity>(["info", "warning", "error", "critical"]);
  const code = input.code === "LOGIN_SUCCESS" && input.outcome === "success"
    ? input.code
    : Object.prototype.hasOwnProperty.call(LOGIN_ERROR_REGISTRY, input.code) ? input.code : "LOGIN_UNKNOWN";
  const correlationId = safeRequestId(input.requestId) ?? createLoginRequestId();
  const httpStatus = Number.isInteger(input.httpStatus) && Number(input.httpStatus) >= 100 && Number(input.httpStatus) <= 599 ? input.httpStatus : undefined;
  const transportCode = typeof input.transportCode === "string" && KNOWN_TRANSPORT_CODES.has(input.transportCode) ? input.transportCode : undefined;
  const serverCode = typeof input.serverCode === "string" && KNOWN_SERVER_CODES.has(input.serverCode) ? input.serverCode : undefined;
  const durationMs = Number.isFinite(input.durationMs) ? Math.max(0, Math.min(Number(input.durationMs), 3_600_000)) : undefined;
  const stage = validStages.has(input.stage) ? input.stage : "unknown";
  const outcome = input.outcome === "success" ? "success" : "failure";
  const event: LoginDiagnosticEvent = {
    schema_version: "1.0",
    timestamp: new Date().toISOString(),
    component: "admin-shell",
    operation: "auth.login",
    correlation_id: correlationId,
    error_code: code,
    // A mapped symptom is not a root-cause diagnosis. Root cause remains unknown
    // until correlated evidence from the Worker or native transport is available.
    cause_confidence: "unknown",
    evidence_source: transportCode ? "transport_code" : serverCode ? "server_code" : httpStatus ? "http_status" : stage === "unknown" ? "unknown" : "client_stage",
    retryability: input.retryable === true ? "yes" : "no",
    remediation_ref: "docs/diagnostics/stage-3-unified-diagnostics.md#login-error-mapping",
    context: {
      ...(httpStatus !== undefined ? { http_status: httpStatus } : {}),
      ...(transportCode ? { transport_code: transportCode } : {}),
      ...(serverCode ? { server_code: serverCode } : {}),
      ...(durationMs !== undefined ? { duration_ms: durationMs } : {}),
    },
    // Compatibility aliases retained for the Stage 1 login event contract.
    event: "onyx.login.diagnostic",
    outcome,
    request_id: correlationId,
    code,
    stage,
    severity: validSeverities.has(input.severity) ? input.severity : "error",
    http_status: httpStatus,
    transport_code: transportCode,
    server_code: serverCode,
    duration_ms: durationMs,
    retryable: input.retryable === true
  };
  try {
    if (typeof window !== "undefined" && typeof window.dispatchEvent === "function") {
      window.dispatchEvent(new CustomEvent<LoginDiagnosticEvent>("onyx:login-diagnostic", { detail: event }));
    }
  } catch {
    // Diagnostics must not break authentication or change its result.
  }
  try {
    console.info("[ONYX_LOGIN_DIAGNOSTIC]", event);
  } catch {
    // Console instrumentation is best-effort only.
  }
}
