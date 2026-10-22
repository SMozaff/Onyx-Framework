import { afterEach, describe, expect, it, vi } from "vitest";
import {
  classifyApiFailure,
  makeApiFailureEvent,
  safeRouteGroup,
  trustedRequestId,
} from "../src/diagnostics";
import worker from "../src/index";

describe("Worker internal diagnostics contract", () => {
  afterEach(() => vi.restoreAllMocks());

  it("uses stable status-based classifications and leaves successful responses unclassified", () => {
    expect(classifyApiFailure(200)).toBeUndefined();
    expect(classifyApiFailure(400)).toBe("API_REQUEST_INVALID");
    expect(classifyApiFailure(401)).toBe("API_AUTHENTICATION_REQUIRED");
    expect(classifyApiFailure(403)).toBe("API_AUTHORIZATION_DENIED");
    expect(classifyApiFailure(404)).toBe("API_NOT_FOUND");
    expect(classifyApiFailure(405)).toBe("API_METHOD_NOT_ALLOWED");
    expect(classifyApiFailure(409)).toBe("API_CONFLICT");
    expect(classifyApiFailure(429)).toBe("API_RATE_LIMITED");
    expect(classifyApiFailure(501)).toBe("API_FEATURE_UNAVAILABLE");
    expect(classifyApiFailure(503)).toBe("API_SERVICE_UNAVAILABLE");
    expect(classifyApiFailure(500)).toBe("API_INTERNAL_FAILURE");
  });

  it("uses bounded route groups without logging dynamic identifiers or arbitrary path segments", () => {
    expect(safeRouteGroup("/api/admin/users/user-secret-id/password")).toBe("/api/admin");
    expect(safeRouteGroup("/api/users/private-user-id")).toBe("/api/users");
    expect(safeRouteGroup("/api/unknown-user-controlled-path")).toBe("/api/other");
    expect(safeRouteGroup("/health")).toBe("/health");
    expect(safeRouteGroup("/some-private-path")).toBe("/other");
  });

  it("preserves valid request IDs and replaces invalid or missing values", () => {
    expect(trustedRequestId("request-123", () => "generated-id")).toBe("request-123");
    expect(trustedRequestId("not a valid id", () => "generated-id")).toBe("generated-id");
    expect(trustedRequestId(null, () => "generated-id")).toBe("generated-id");
  });

  it("bounds event fields and omits any event for a successful response", () => {
    const event = makeApiFailureEvent({
      requestId: "request-123",
      pathname: "/api/admin/users/private-id",
      method: "DELETE",
      status: 403,
      elapsedMs: 10,
      originAllowed: false,
    });
    expect(event).toMatchObject({
      schema_version: "1.0",
      severity: "warning",
      component: "cloudflare-worker",
      operation: "DELETE /api/admin",
      stage: "worker",
      correlation_id: "request-123",
      outcome: "failure",
      cause_confidence: "unknown",
      evidence_source: "http_status",
      retryability: "no",
      remediation_ref: "docs/diagnostics/stage-3-unified-diagnostics.md#worker-http-error-mapping",
      context: {
        route_group: "/api/admin",
        method: "DELETE",
        http_status: 403,
        duration_ms: 10,
        origin_allowed: false,
      },
      event: "onyx.api.failure",
      request_id: "request-123",
      route_group: "/api/admin",
      method: "DELETE",
      status: 403,
      diagnostic_code: "API_AUTHORIZATION_DENIED",
      elapsed_ms: 10,
      origin_allowed: false,
    });
    expect(event?.timestamp).toMatch(/^\\d{4}-\\d{2}-\\d{2}T/);
    expect(JSON.stringify(event)).not.toContain("private-id");
    expect(JSON.stringify(event)).not.toContain("private payload");
    expect(makeApiFailureEvent({
      requestId: "request-123",
      pathname: "/api/query",
      method: "GET",
      status: 200,
      elapsedMs: 1,
      originAllowed: true,
    })).toBeUndefined();
  });

  it("maps response classes to conservative severity and retryability without inferring root cause", () => {
    expect(makeApiFailureEvent({ requestId: "request-1", pathname: "/api/query", method: "GET", status: 503, elapsedMs: 20, originAllowed: true }))
      .toMatchObject({ severity: "error", retryability: "yes", cause_confidence: "unknown", evidence_source: "http_status" });
    expect(makeApiFailureEvent({ requestId: "request-2", pathname: "/api/query", method: "GET", status: 500, elapsedMs: 20, originAllowed: true }))
      .toMatchObject({ severity: "error", retryability: "unknown", cause_confidence: "unknown" });
  });

  it("propagates request IDs to non-login HTTP errors without logging credentials or raw paths", async () => {
    const logged: string[] = [];
    vi.spyOn(console, "error").mockImplementation((value?: unknown) => {
      logged.push(String(value));
    });
    const secret = "do-not-log-this-bearer";
    const response = await worker.fetch(new Request("https://worker.test/api/users/private-user-id", {
      headers: {
        "X-ONYX-Request-ID": "stage2-request-001",
        authorization: `Bearer ${secret}`,
        origin: "https://app.example",
      },
    }), {
      DB: {} as unknown as D1Database,
      // The request deliberately carries a malformed bearer credential. Configure
      // the test Worker so jwtVerify reaches token validation instead of failing
      // early with "ONYX_JWT_SECRET is not configured" (HTTP 500).
      ONYX_JWT_SECRET: "diagnostics-test-jwt-secret",
      ONYX_CORS_ORIGINS: "https://app.example",
    } as never);

    expect(response.status).toBe(401);
    expect(response.headers.get("x-onyx-request-id")).toBe("stage2-request-001");
    expect(response.headers.get("access-control-allow-origin")).toBe("https://app.example");
    const body = await response.json() as Record<string, unknown>;
    expect(body.correlation_id).toBe("stage2-request-001");

    expect(logged).toHaveLength(1);
    const event = JSON.parse(logged[0]) as Record<string, unknown>;
    expect(event).toMatchObject({
      event: "onyx.api.failure",
      request_id: "stage2-request-001",
      route_group: "/api/users",
      status: 401,
      diagnostic_code: "API_AUTHENTICATION_REQUIRED",
    });
    expect(logged.join("\n")).not.toContain(secret);
    expect(logged.join("\n")).not.toContain("private-user-id");
    expect(logged.join("\n")).not.toContain("https://app.example");
  });

  it("records dependency failures without exposing raw database errors", async () => {
    const logged: string[] = [];
    vi.spyOn(console, "error").mockImplementation((value?: unknown) => {
      logged.push(String(value));
    });
    const privateFailure = "private SQL detail and secret marker";
    const response = await worker.fetch(new Request("https://worker.test/ready", {
      headers: { "X-ONYX-Request-ID": "ready-failure-001" },
    }), {
      DB: {
        prepare: () => { throw new Error(privateFailure); },
      } as unknown as D1Database,
      ONYX_CORS_ORIGINS: "https://app.example",
    } as never);

    expect(response.status).toBe(503);
    expect(response.headers.get("x-onyx-request-id")).toBe("ready-failure-001");
    expect(await response.json()).toMatchObject({
      status: "degraded",
      database: "unavailable",
      schema: "unknown",
    });
    expect(logged).toHaveLength(1);
    expect(JSON.parse(logged[0])).toMatchObject({
      event: "onyx.api.failure",
      request_id: "ready-failure-001",
      route_group: "/ready",
      status: 503,
      diagnostic_code: "API_SERVICE_UNAVAILABLE",
    });
    expect(logged.join("\\n")).not.toContain(privateFailure);
  });

  it("replaces invalid incoming request IDs and does not widen CORS origins", async () => {
    const response = await worker.fetch(new Request("https://worker.test/api/query", {
      headers: {
        "X-ONYX-Request-ID": "bad request id",
        origin: "https://evil.example",
      },
    }), {
      DB: {} as unknown as D1Database,
      ONYX_CORS_ORIGINS: "https://app.example",
    } as never);

    expect(response.status).toBe(401);
    const requestId = response.headers.get("x-onyx-request-id");
    expect(requestId).toMatch(/^[A-Za-z0-9._:-]{1,80}$/);
    expect(requestId).not.toBe("bad request id");
    expect(response.headers.get("access-control-allow-origin")).toBeNull();
  });
});
