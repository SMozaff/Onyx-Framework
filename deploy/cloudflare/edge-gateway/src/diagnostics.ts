/**
 * Safe request-boundary diagnostics for the Cloudflare Worker.
 *
 * Keep this transport-neutral with respect to the browser login registry:
 * the Worker owns HTTP correlation and emits only stable classifications,
 * bounded route groups, status, duration and booleans. Never pass payloads,
 * credentials, raw URLs, exception messages or arbitrary headers here.
 */

const REQUEST_ID_PATTERN = /^[A-Za-z0-9._:-]{1,80}$/;

export type ApiFailureCode =
  | "API_REQUEST_INVALID"
  | "API_AUTHENTICATION_REQUIRED"
  | "API_AUTHORIZATION_DENIED"
  | "API_NOT_FOUND"
  | "API_METHOD_NOT_ALLOWED"
  | "API_CONFLICT"
  | "API_RATE_LIMITED"
  | "API_FEATURE_UNAVAILABLE"
  | "API_SERVICE_UNAVAILABLE"
  | "API_INTERNAL_FAILURE"
  | "API_REQUEST_REJECTED";

export function trustedRequestId(value: string | null, generate: () => string = () => crypto.randomUUID()): string {
  return value && REQUEST_ID_PATTERN.test(value) ? value : generate();
}

const ROUTE_GROUPS = new Set([
  "auth", "admin", "users", "query", "command", "relay", "events",
  "profiles", "files", "push", "todo", "allfather",
]);

/** Returns a low-cardinality route family; never includes IDs, query values or arbitrary path segments. */
export function safeRouteGroup(pathname: string): string {
  const segments = pathname.split("/").filter(Boolean);
  if (segments.length === 0) return "/";
  if (segments[0] !== "api") return segments[0] === "health" || segments[0] === "ready" ? `/${segments[0]}` : "/other";
  const family = segments[1] ?? "";
  if (!ROUTE_GROUPS.has(family)) return "/api/other";
  return `/api/${family}`;
}

export function classifyApiFailure(status: number): ApiFailureCode | undefined {
  if (status < 400) return undefined;
  if (status === 400 || status === 422) return "API_REQUEST_INVALID";
  if (status === 401) return "API_AUTHENTICATION_REQUIRED";
  if (status === 403) return "API_AUTHORIZATION_DENIED";
  if (status === 404) return "API_NOT_FOUND";
  if (status === 405) return "API_METHOD_NOT_ALLOWED";
  if (status === 409) return "API_CONFLICT";
  if (status === 429) return "API_RATE_LIMITED";
  if (status === 501) return "API_FEATURE_UNAVAILABLE";
  if (status === 502 || status === 503 || status === 504) return "API_SERVICE_UNAVAILABLE";
  if (status >= 500) return "API_INTERNAL_FAILURE";
  return "API_REQUEST_REJECTED";
}

export type SafeCauseType = "TypeError" | "RangeError" | "SyntaxError" | "Error" | "UnknownError";

export type DiagnosticSeverity = "info" | "warning" | "error" | "critical";
export type DiagnosticRetryability = "yes" | "no" | "unknown";

export interface ApiFailureEvent {
  schema_version: "1.0";
  timestamp: string;
  severity: DiagnosticSeverity;
  component: "cloudflare-worker";
  operation: string;
  stage: "worker";
  correlation_id: string;
  outcome: "failure";
  error_code: ApiFailureCode;
  cause_confidence: "unknown";
  evidence_source: "http_status";
  retryability: DiagnosticRetryability;
  remediation_ref: string;
  context: {
    route_group: string;
    method: string;
    http_status: number;
    duration_ms: number;
    origin_allowed: boolean | null;
    cause_type?: SafeCauseType;
  };
  // Compatibility aliases retained for existing Stage 2 log queries.
  event: "onyx.api.failure";
  request_id: string;
  route_group: string;
  method: string;
  status: number;
  diagnostic_code: ApiFailureCode;
  elapsed_ms: number;
  origin_allowed: boolean | null;
  cause_type?: SafeCauseType;
}

function severityForStatus(status: number): DiagnosticSeverity {
  if (status >= 500) return "error";
  if (status >= 400) return "warning";
  return "info";
}

function retryabilityForStatus(status: number): DiagnosticRetryability {
  if (status === 429 || status === 502 || status === 503 || status === 504) return "yes";
  if ([400, 401, 403, 404, 405, 409, 422, 501].includes(status)) return "no";
  return "unknown";
}

const LOGIN_SERVER_CODE_MAP: Record<string, string> = {
  INVALID_LOGIN_REQUEST: "LOGIN_REQUEST_INVALID",
  INVALID_CLIENT_TYPE: "LOGIN_CLIENT_TYPE_INVALID",
  INVALID_CREDENTIALS: "LOGIN_AUTH_REJECTED",
  LOGIN_SESSION_CREATION_FAILED: "LOGIN_SESSION_CREATION_FAILED",
  LOGIN_BACKEND_FAILURE: "LOGIN_BACKEND_FAILURE",
  INTERNAL_SERVER_ERROR: "LOGIN_BACKEND_FAILURE",
  METHOD_NOT_ALLOWED: "LOGIN_METHOD_NOT_ALLOWED",
  AUTH_ROUTE_NOT_FOUND: "LOGIN_ROUTE_NOT_FOUND",
  ORIGIN_NOT_ALLOWED: "LOGIN_CORS_ORIGIN_REJECTED",
  FORBIDDEN: "LOGIN_AUTHORIZATION_DENIED",
  RATE_LIMITED: "LOGIN_RATE_LIMITED",
};
const SAFE_LOGIN_CODES = new Set([
  ...Object.keys(LOGIN_SERVER_CODE_MAP),
  "LOGIN_SUCCESS", "LOGIN_HTTP_ERROR", "LOGIN_UNKNOWN", "LOGIN_REQUEST_INVALID",
  "LOGIN_CLIENT_TYPE_INVALID", "LOGIN_AUTH_REJECTED", "LOGIN_METHOD_NOT_ALLOWED",
  "LOGIN_ROUTE_NOT_FOUND", "LOGIN_AUTHORIZATION_DENIED", "LOGIN_RATE_LIMITED",
  "LOGIN_CORS_ORIGIN_REJECTED", "LOGIN_BACKEND_FAILURE", "LOGIN_SESSION_CREATION_FAILED",
]);

export interface WorkerLoginDiagnosticInput {
  event?: "onyx.login.http" | "onyx.login.failure";
  requestId: string | null;
  legacyStage: string;
  method: string;
  status: number;
  outcome: "success" | "failure";
  serverCode?: string;
  legacyErrorCode?: string;
  legacyOutcome?: "success" | "http_error" | "cors_rejected";
  diagnosticCode?: string;
  elapsedMs?: number;
  origin?: string | null;
  originAllowed: boolean | null;
  causeType?: SafeCauseType;
  corsRejected?: boolean;
}

/** Canonicalizes Stage 1 Worker login logs while retaining their prior values under legacy_* keys. */
export function makeWorkerLoginDiagnosticEvent(input: WorkerLoginDiagnosticInput): Record<string, unknown> {
  const method = /^(GET|POST|PUT|PATCH|DELETE|OPTIONS|HEAD)$/.test(input.method) ? input.method : "OTHER";
  const status = Number.isInteger(input.status) && input.status >= 100 && input.status <= 599 ? input.status : 500;
  const legacyStage = /^[a-z_]{1,64}$/.test(input.legacyStage) ? input.legacyStage : "unknown";
  const serverCode = typeof input.serverCode === "string" && Object.prototype.hasOwnProperty.call(LOGIN_SERVER_CODE_MAP, input.serverCode)
    ? input.serverCode
    : undefined;
  const diagnosticCode = input.corsRejected
    ? "LOGIN_CORS_ORIGIN_REJECTED"
    : input.outcome === "success" && status < 400
      ? "LOGIN_SUCCESS"
      : serverCode
        ? LOGIN_SERVER_CODE_MAP[serverCode]
        : status === 404
          ? "LOGIN_ROUTE_NOT_FOUND"
          : status === 405
            ? "LOGIN_METHOD_NOT_ALLOWED"
            : status === 429
              ? "LOGIN_RATE_LIMITED"
              : status >= 500
                ? "LOGIN_BACKEND_FAILURE"
                : "LOGIN_HTTP_ERROR";
  const stage = ({
    configuration: "configuration",
    transport: "transport",
    cors: "cors",
    cors_preflight: "cors",
    routing: "routing",
    request_validation: "request_validation",
    credential_lookup: "authentication",
    credential_verification: "authentication",
    authentication: "authentication",
    authentication_response: "authentication",
    authorization: "authorization",
    backend: "backend",
    worker_dispatch: "backend",
    dependency: "dependency",
    session: "session",
    session_creation: "session",
    session_issued: "session",
    client: "client",
    navigation: "navigation",
    unknown: "unknown",
  } as Record<string, string>)[legacyStage] ?? "unknown";
  const originText = typeof input.origin === "string"
    ? input.origin.slice(0, 200).replace(/[\x00-\x1f\x7f]/g, "?")
    : null;
  const originSeparator = originText?.indexOf("://") ?? -1;
  const originScheme = originSeparator > 0 ? originText?.slice(0, originSeparator) ?? "" : "";
  const originHost = originSeparator > 0 ? originText?.slice(originSeparator + 3) ?? "" : "";
  const validOrigin = originText === "null" ||
    (/^[A-Za-z][A-Za-z0-9+.-]*$/.test(originScheme) &&
     /^(?:[A-Za-z0-9.-]+|\[[0-9A-Fa-f:.]+\])(?::[0-9]{1,5})?$/.test(originHost));
  const origin = originText === null ? null : validOrigin ? originText : "[invalid-origin]";
  const elapsedMs = Number.isFinite(input.elapsedMs)
    ? Math.max(0, Math.min(Math.trunc(Number(input.elapsedMs)), 86_400_000))
    : undefined;
  const severity = input.outcome === "success" ? "info" : status >= 500 ? "error" : "warning";
  const retryability = input.outcome === "success" ||
      diagnosticCode === "LOGIN_CORS_ORIGIN_REJECTED"
    ? "no"
    : status === 429 || [502, 503, 504].includes(status) ||
        diagnosticCode === "LOGIN_BACKEND_FAILURE" ||
        diagnosticCode === "LOGIN_SESSION_CREATION_FAILED" ||
        diagnosticCode === "LOGIN_RATE_LIMITED"
      ? "yes"
      : status >= 400 && status < 500
        ? "no"
        : "unknown";
  const safeCauseType = input.causeType && ["TypeError", "RangeError", "SyntaxError", "Error", "UnknownError"].includes(input.causeType)
    ? input.causeType
    : undefined;
  const legacyErrorCode = input.legacyErrorCode && SAFE_LOGIN_CODES.has(input.legacyErrorCode)
    ? input.legacyErrorCode
    : undefined;
  const requestId = trustedRequestId(input.requestId);
  const context = {
    route_group: "/api/auth",
    method,
    http_status: status,
    ...(elapsedMs !== undefined ? { duration_ms: elapsedMs } : {}),
    origin_allowed: input.originAllowed,
    ...(serverCode ? { server_code: serverCode } : {}),
    ...(safeCauseType ? { cause_type: safeCauseType } : {}),
  };
  return {
    schema_version: "1.0",
    timestamp: new Date().toISOString(),
    severity,
    component: "cloudflare-worker",
    operation: method + " /api/auth/login",
    stage,
    correlation_id: requestId,
    outcome: input.outcome,
    error_code: diagnosticCode,
    cause_confidence: "unknown",
    evidence_source: input.corsRejected ? "cors_policy" : serverCode ? "server_code" : "http_status",
    retryability,
    remediation_ref: "docs/diagnostics/stage-3-unified-diagnostics.md#login-error-mapping",
    context,
    event: input.event ?? "onyx.login.http",
    request_id: requestId,
    legacy_stage: legacyStage,
    method,
    status,
    ...(input.legacyOutcome ? { legacy_outcome: input.legacyOutcome } : {}),
    ...(legacyErrorCode ? { legacy_error_code: legacyErrorCode } : {}),
    ...(input.diagnosticCode && /^(API|LOGIN)_[A-Z_]+$/.test(input.diagnosticCode) ? { diagnostic_code: input.diagnosticCode } : {}),
    ...(elapsedMs !== undefined ? { elapsed_ms: elapsedMs } : {}),
    origin,
    origin_allowed: input.originAllowed,
    ...(safeCauseType ? { cause_type: safeCauseType } : {}),
  };
}

export function makeApiFailureEvent(input: {
  requestId: string;
  pathname: string;
  method: string;
  status: number;
  elapsedMs: number;
  originAllowed: boolean | null;
  causeType?: SafeCauseType;
}): ApiFailureEvent | undefined {
  const diagnosticCode = classifyApiFailure(input.status);
  if (!diagnosticCode) return undefined;
  const method = /^(GET|POST|PUT|PATCH|DELETE|OPTIONS|HEAD)$/.test(input.method) ? input.method : "OTHER";
  const requestId = trustedRequestId(input.requestId);
  const routeGroup = safeRouteGroup(input.pathname);
  const elapsedMs = Math.max(0, Math.min(Math.trunc(input.elapsedMs), 86_400_000));
  const context = {
    route_group: routeGroup,
    method,
    http_status: input.status,
    duration_ms: elapsedMs,
    origin_allowed: input.originAllowed,
    ...(input.causeType ? { cause_type: input.causeType } : {}),
  };
  return {
    schema_version: "1.0",
    timestamp: new Date().toISOString(),
    severity: severityForStatus(input.status),
    component: "cloudflare-worker",
    operation: `${method} ${routeGroup}`,
    // Stage 2 reports at the Worker response boundary; a status does not prove
    // which internal handler or dependency caused the failure.
    stage: "worker",
    correlation_id: requestId,
    outcome: "failure",
    error_code: diagnosticCode,
    cause_confidence: "unknown",
    evidence_source: "http_status",
    retryability: retryabilityForStatus(input.status),
    remediation_ref: "docs/diagnostics/stage-3-unified-diagnostics.md#worker-http-error-mapping",
    context,
    // Compatibility aliases retained for existing Stage 2 log queries.
    event: "onyx.api.failure",
    request_id: requestId,
    route_group: routeGroup,
    method,
    status: input.status,
    diagnostic_code: diagnosticCode,
    elapsed_ms: elapsedMs,
    origin_allowed: input.originAllowed,
    ...(input.causeType ? { cause_type: input.causeType } : {}),
  };
}
