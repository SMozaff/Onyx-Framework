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

export interface ApiFailureEvent {
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
  return {
    event: "onyx.api.failure",
    request_id: trustedRequestId(input.requestId, () => "invalid-request-id"),
    route_group: safeRouteGroup(input.pathname),
    method,
    status: input.status,
    diagnostic_code: diagnosticCode,
    elapsed_ms: Math.max(0, Math.min(Math.trunc(input.elapsedMs), 86_400_000)),
    origin_allowed: input.originAllowed,
    ...(input.causeType ? { cause_type: input.causeType } : {}),
  };
}
