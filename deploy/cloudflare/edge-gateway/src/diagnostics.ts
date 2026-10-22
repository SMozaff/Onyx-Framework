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
  if (status === 401 || status === 403 || status === 429) return "warning";
  return "info";
}

function retryabilityForStatus(status: number): DiagnosticRetryability {
  if (status === 429 || status === 502 || status === 503 || status === 504) return "yes";
  if ([400, 401, 403, 404, 405, 409, 422, 501].includes(status)) return "no";
  return "unknown";
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
