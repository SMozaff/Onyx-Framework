import { afterEach, describe, expect, it, vi } from "vitest";
import {
  LOGIN_ERROR_REGISTRY,
  classifyLoginFailure,
  formatLoginFailureMessage,
  getLoginConfigurationFailure,
  loginFailureForCode,
  recordLoginDiagnostic,
  type LoginErrorCode,
} from "../src/auth/loginDiagnostics";

function axiosError(options: {
  code?: string;
  status?: number;
  serverCode?: string;
} = {}) {
  return {
    isAxiosError: true,
    code: options.code,
    response: options.status === undefined ? undefined : {
      status: options.status,
      data: options.serverCode ? { error: options.serverCode } : {},
      headers: { "x-onyx-request-id": "worker-request-123" },
    },
  };
}

describe("Admin login diagnostics registry", () => {
  afterEach(() => vi.restoreAllMocks());

  it("defines the required safe metadata for every stable code", () => {
    const required = [
      "category", "stage", "severity", "userMessage", "internalDiagnostic",
      "retryable", "retryGuidance", "evidenceRequired", "remediation", "observability",
    ] as const;
    const codes = Object.keys(LOGIN_ERROR_REGISTRY) as LoginErrorCode[];
    expect(codes.length).toBeGreaterThanOrEqual(20);
    for (const code of codes) {
      for (const field of required) {
        const value = LOGIN_ERROR_REGISTRY[code][field];
        expect(value, code + "." + field).toBeDefined();
        if (typeof value === "string") expect(value.trim().length, code + "." + field).toBeGreaterThan(0);
        else expect(typeof value, code + "." + field).toBe("boolean");
      }
    }
  });

  it("distinguishes missing configuration from an unsupported or insecure endpoint", () => {
    expect(getLoginConfigurationFailure("")).toBe("LOGIN_CONFIG_MISSING");
    expect(getLoginConfigurationFailure(undefined)).toBe("LOGIN_CONFIG_MISSING");
    expect(getLoginConfigurationFailure("http://localhost:5174")).toBe("LOGIN_CONFIG_INVALID");
    expect(getLoginConfigurationFailure("https://not-the-configured-endpoint.example")).toBe("LOGIN_CONFIG_INVALID");
    expect(getLoginConfigurationFailure("https://onyx-framework.soheil-mozaffari.workers.dev")).toBeNull();
  });

  it("creates valid request IDs for correlation", () => {
    expect(classifyLoginFailure(axiosError({ code: "ERR_NETWORK" }), "client-id").requestId).toBe("client-id");
  });

  it("classifies only explicit timeout, DNS, TLS, cancellation, and connection transport codes", () => {
    expect(classifyLoginFailure(axiosError({ code: "ETIMEDOUT" })).code).toBe("LOGIN_TIMEOUT");
    expect(classifyLoginFailure(axiosError({ code: "ENOTFOUND" })).code).toBe("LOGIN_DNS_FAILURE");
    expect(classifyLoginFailure(axiosError({ code: "ERR_TLS_CERT_ALTNAME_INVALID" })).code).toBe("LOGIN_TLS_FAILURE");
    expect(classifyLoginFailure(axiosError({ code: "ECONNREFUSED" })).code).toBe("LOGIN_CONNECTION_FAILURE");
    expect(classifyLoginFailure(axiosError({ code: "ERR_CANCELED" })).code).toBe("LOGIN_REQUEST_CANCELLED");
  });

  it("keeps opaque no-response errors unclassified as CORS", () => {
    const failure = classifyLoginFailure(axiosError({ code: "ERR_NETWORK" }), "client-id");
    expect(failure.code).toBe("LOGIN_RESPONSE_UNAVAILABLE");
    expect(failure.requestId).toBe("client-id");
    expect(failure.code).not.toBe("LOGIN_CORS_ORIGIN_REJECTED");
  });

  it("uses the response request ID and a safe machine code for invalid credentials", () => {
    const failure = classifyLoginFailure(axiosError({ status: 401, serverCode: "INVALID_CREDENTIALS" }), "client-id");
    expect(failure).toMatchObject({
      code: "LOGIN_AUTH_REJECTED",
      requestId: "worker-request-123",
      httpStatus: 401,
      serverCode: "INVALID_CREDENTIALS",
      retryable: false,
    });
    expect(failure.userMessage).toBe("The username or password is incorrect.");
  });

  it("maps malformed request, client type, CORS, routing, rate limit, authorization, and backend evidence", () => {
    expect(classifyLoginFailure(axiosError({ status: 400, serverCode: "INVALID_LOGIN_REQUEST" })).code).toBe("LOGIN_REQUEST_INVALID");
    expect(classifyLoginFailure(axiosError({ status: 400, serverCode: "INVALID_CLIENT_TYPE" })).code).toBe("LOGIN_CLIENT_TYPE_INVALID");
    expect(classifyLoginFailure(axiosError({ status: 403, serverCode: "ORIGIN_NOT_ALLOWED" })).code).toBe("LOGIN_CORS_ORIGIN_REJECTED");
    expect(classifyLoginFailure(axiosError({ status: 403, serverCode: "FORBIDDEN" })).code).toBe("LOGIN_AUTHORIZATION_DENIED");
    expect(classifyLoginFailure(axiosError({ status: 405, serverCode: "METHOD_NOT_ALLOWED" })).code).toBe("LOGIN_METHOD_NOT_ALLOWED");
    expect(classifyLoginFailure(axiosError({ status: 404, serverCode: "AUTH_ROUTE_NOT_FOUND" })).code).toBe("LOGIN_ROUTE_NOT_FOUND");
    expect(classifyLoginFailure(axiosError({ status: 429 })).code).toBe("LOGIN_RATE_LIMITED");
    expect(classifyLoginFailure(axiosError({ status: 503 })).code).toBe("LOGIN_BACKEND_FAILURE");
    expect(classifyLoginFailure(axiosError({ status: 400 })).code).toBe("LOGIN_HTTP_ERROR");
  });

  it("distinguishes session creation, malformed success responses, and explicit parsing exceptions", () => {
    expect(classifyLoginFailure(axiosError({ status: 500, serverCode: "LOGIN_SESSION_CREATION_FAILED" })).code).toBe("LOGIN_SESSION_CREATION_FAILED");
    expect(classifyLoginFailure(new Error("INVALID_AUTH_RESPONSE")).code).toBe("LOGIN_UNEXPECTED_RESPONSE");
    expect(classifyLoginFailure(new SyntaxError("private parser detail")).code).toBe("LOGIN_CLIENT_PARSING_FAILED");
  });

  it("uses unknown fallback for unrelated JavaScript errors rather than calling them network failures", () => {
    const failure = classifyLoginFailure(new Error("private-exception-message"));
    expect(failure.code).toBe("LOGIN_UNKNOWN");
    expect(failure.userMessage).not.toContain("private-exception-message");
  });

  it("records only the allowlisted diagnostic fields and never the supplied exception text", () => {
    const spy = vi.spyOn(console, "info").mockImplementation(() => undefined);
    const failure = classifyLoginFailure(axiosError({ code: "ERR_NETWORK" }), "safe-correlation-id");
    recordLoginDiagnostic({
      outcome: "failure",
      code: failure.code,
      stage: failure.stage,
      severity: failure.severity,
      requestId: failure.requestId,
      httpStatus: failure.httpStatus,
      transportCode: failure.transportCode,
      serverCode: failure.serverCode,
      retryable: failure.retryable,
    });
    // Runtime callers can still pass untrusted values despite the TypeScript
    // contract. The event writer must independently enforce its allowlist.
    recordLoginDiagnostic({
      outcome: "failure",
      code: "password=untrusted-private-detail",
      stage: "transport",
      severity: "error",
      requestId: "password=untrusted-private-detail",
      transportCode: "secret-derived-password-code",
      serverCode: "access_token=untrusted",
      retryable: false,
    });
    const serialized = JSON.stringify(spy.mock.calls);
    for (const sensitive of ["password", "access_token", "refresh_token", "secret-environment-value", "private-exception-message"]) {
      expect(serialized.toLowerCase()).not.toContain(sensitive);
    }
    expect(serialized).toContain("safe-correlation-id");
    expect(serialized).toContain("LOGIN_RESPONSE_UNAVAILABLE");
  });

  it("formats a safe machine code and reference ID for the login error UI", () => {
    const failure = loginFailureForCode("LOGIN_AUTH_REJECTED", "request-id-006");
    expect(formatLoginFailureMessage(failure)).toBe(
      "The username or password is incorrect. (LOGIN_AUTH_REJECTED; reference request-id-006)",
    );
    expect(formatLoginFailureMessage(failure, "fallback-id", "Localized denial"))
      .toBe("Localized denial (LOGIN_AUTH_REJECTED; reference request-id-006)");
    expect(formatLoginFailureMessage({ ...failure, requestId: "password=secret" }, "fallback-id"))
      .toContain("reference fallback-id");
  });

  it("does not return raw exceptions in user-facing failures", () => {
    const failure = loginFailureForCode("LOGIN_BACKEND_FAILURE", "request-id");
    expect(failure.userMessage).not.toContain("ONYX_JWT_SECRET");
    expect(failure.userMessage).not.toContain("password_hash");
    expect(failure).not.toHaveProperty("exception");
    expect(failure).not.toHaveProperty("body");
  });
});
