/**
 * Authentication contract tests for ONYX username/password login.
 * Google/Supabase is not an All-Father authentication path.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import { argon2id } from "hash-wasm";
import worker from "../src/index";
import { AUTH_SCHEMA_CONTRACT } from "../src/schema-contract";

const ALLFATHER_USERNAME = "allfather";

const fetchHandler = worker.fetch;
type WorkerEnv = Parameters<typeof fetchHandler>[1];

interface RecordedQuery {
  sql: string;
  params: unknown[];
}

class MockDb {
  userRow: Record<string, unknown> | null = null;
  queries: RecordedQuery[] = [];
  missingSchemaItems = new Set<string>();

  private schemaRows(sql: string): Array<Record<string, unknown>> {
    const tablePrefix = "PRAGMA table_info(";
    if (sql.toLowerCase().startsWith(tablePrefix.toLowerCase()) && sql.endsWith(")")) {
      const table = sql.slice(tablePrefix.length, -1);
      if (this.missingSchemaItems.has(`table:${table}`)) return [];
      return (AUTH_SCHEMA_CONTRACT.tables[table] ?? [])
        .filter((column) => !this.missingSchemaItems.has(`${table}.${column}`))
        .map((name) => ({ name }));
    }
    const compactSql = sql.toLowerCase().replaceAll(" ", "");
    const indexPrefix = "pragmaindex_list(";
    if (compactSql.startsWith(indexPrefix) && compactSql.endsWith(")")) {
      const table = compactSql.slice(indexPrefix.length, -1);
      const indexes = table === "users"
        ? AUTH_SCHEMA_CONTRACT.indexes
        : AUTH_SCHEMA_CONTRACT.tableIndexes[table] ?? [];
      return indexes
        .filter((name) => !this.missingSchemaItems.has(`index:${name}`))
        .map((name) => ({ name }));
    }
    return [];
  }

  prepare(sql: string) {
    const record = (params: unknown[]) => this.queries.push({ sql, params });
    const row = () => (sql.includes("FROM users") ? this.userRow : null);
    return {
      // D1 exposes .all() directly on a prepared statement; readiness uses
      // this to inspect PRAGMA schema rows without binding parameters.
      all: async () => ({ results: this.schemaRows(sql) }),
      bind: (...params: unknown[]) => {
        record(params);
        return {
          first: async () => row(),
          run: async () => ({ success: true }),
          all: async () => ({ results: this.schemaRows(sql) }),
        };
      },
      first: async () => {
        record([]);
        return row();
      },
    };
  }

  writes(): RecordedQuery[] {
    return this.queries.filter((q) => /^\s*(UPDATE|INSERT|DELETE)/i.test(q.sql));
  }
}

function makeEnv(db: MockDb): WorkerEnv {
  return {
    DB: db,
    ONYX_JWT_SECRET: "test-onyx-secret",
    ONYX_CORS_ORIGINS: "https://app.example",
  } as unknown as WorkerEnv;
}

function allfatherRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "__allfather__",
    username: "allfather",
    email: "allfather@example.test",
    password_hash: "",
    organization_id: "__allfather__",
    is_admin: 1,
    is_active: 1,
    class: "ALL_FATHER",
    role: "ALL_FATHER",
    ...overrides,
  };
}

describe("POST /api/auth/login (ONYX password path)", () => {
  async function login(identifier: string, password: string) {
    const db = new MockDb();
    const request = new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ username: identifier, password }),
    });
    const response = await fetchHandler(request, makeEnv(db));
    return { response, body: await response.json() as Record<string, unknown> };
  }

  it("generic-fails for an unknown identifier", async () => {
    const { response, body } = await login("no-such-user", "whatever");
    expect(response.status).toBe(401);
    expect(body.error).toBe("INVALID_CREDENTIALS");
  });

  it("never infers ALL_FATHER from the username when role is empty (Phase 4)", async () => {
    // The canonical seed always sets role explicitly. A row whose role is empty
    // must not be promoted by the literal username `allfather`; it is treated as
    // an ordinary account (a fail-closed downgrade, never an implicit grant).
    const hash = await argon2id({
      password: "correct horse battery staple",
      salt: new Uint8Array(16).fill(9),
      parallelism: 1,
      iterations: 1,
      memorySize: 8192,
      hashLength: 32,
      outputType: "encoded",
    });
    const db = new MockDb();
    db.userRow = {
      id: "legacy-allfather-name",
      username: "allfather",
      email: "someone@example.test",
      organization_id: "org-1",
      password_hash: hash,
      is_admin: 0,
      is_active: 1,
      class: null,
      role: "",
    };
    const request = new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ username: "allfather", password: "correct horse battery staple" }),
    });

    const response = await fetchHandler(request, makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(401);
    expect(body.error).toBe("INVALID_CREDENTIALS");
    expect(body.access_token).toBeUndefined();
    expect(body.refresh_token).toBeUndefined();
  });

  it("returns an identical response for unknown identifier and wrong password", async () => {
    const hash = await argon2id({
      password: "correct horse battery staple",
      salt: new Uint8Array(16).fill(7),
      parallelism: 1,
      iterations: 1,
      memorySize: 8192,
      hashLength: 32,
      outputType: "encoded",
    });

    const db = new MockDb();
    db.userRow = {
      id: "admin-1",
      username: "admin1",
      email: "admin1@example.test",
      organization_id: "org-1",
      password_hash: hash,
      is_admin: 1,
      is_active: 1,
      class: null,
      role: "ORGANIZATION_ADMIN",
    };
    const request = new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ username: "admin1", password: "wrong-password" }),
    });
    const wrongPassword = await fetchHandler(request, makeEnv(db));
    const wrongBody = await wrongPassword.json() as Record<string, unknown>;

    const { response: unknownUser, body } = await login("no-such-user", "wrong-password");

    expect(unknownUser.status).toBe(401);
    expect(wrongPassword.status).toBe(401);

    // The public authentication contract must be identical except for the
    // per-request correlation ID, which must remain unique for diagnostics.
    const { correlation_id: unknownCorrelationId, ...unknownPublicBody } = body;
    const { correlation_id: wrongCorrelationId, ...wrongPublicBody } = wrongBody;
    expect(unknownPublicBody).toEqual(wrongPublicBody);
    expect(unknownPublicBody).toEqual({
      error: "INVALID_CREDENTIALS",
      code: "INVALID_CREDENTIALS",
      category: "AUTHORITY",
      retryability: "NON_RETRYABLE",
    });
    expect(unknownCorrelationId).toMatch(/^[A-Za-z0-9._:-]{1,80}$/);
    expect(wrongCorrelationId).toMatch(/^[A-Za-z0-9._:-]{1,80}$/);
    expect(unknownCorrelationId).not.toBe(wrongCorrelationId);
  });
});



describe("POST /api/auth/login (canonical All-Father)", () => {
  it("issues an ONYX session only for the explicitly provisioned canonical principal", async () => {
    const password = "correct horse battery staple";
    const hash = await argon2id({ password, salt: new Uint8Array(16).fill(11), parallelism: 1, iterations: 1, memorySize: 8192, hashLength: 32, outputType: "encoded" });
    const db = new MockDb();
    db.userRow = allfatherRow({ password_hash: hash });
    const request = new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ username: ALLFATHER_USERNAME, password, client_type: "admin" }),
    });
    const response = await fetchHandler(request, makeEnv(db));
    const body = await response.json() as Record<string, unknown>;
    const user = body.user as Record<string, unknown>;
    expect(response.status).toBe(200);
    expect(body.access_token).toEqual(expect.any(String));
    expect(body.refresh_token).toEqual(expect.any(String));
    expect(user.id).toBe("__allfather__");
    expect(user.role).toBe("ALL_FATHER");
    expect(user.is_admin).toBe(true);
    expect(db.writes()).toHaveLength(0);
  });

  it("denies the canonical principal if the explicit role or admin flag is missing", async () => {
    const password = "correct horse battery staple";
    const hash = await argon2id({ password, salt: new Uint8Array(16).fill(12), parallelism: 1, iterations: 1, memorySize: 8192, hashLength: 32, outputType: "encoded" });
    const db = new MockDb();
    db.userRow = allfatherRow({ password_hash: hash, role: "", is_admin: 0 });
    const request = new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ username: ALLFATHER_USERNAME, password, client_type: "admin" }),
    });
    const response = await fetchHandler(request, makeEnv(db));
    const body = await response.json() as Record<string, unknown>;
    expect(response.status).toBe(401);
    expect(body.error).toBe("INVALID_CREDENTIALS");
    expect(body.access_token).toBeUndefined();
    expect(body.refresh_token).toBeUndefined();
  });
});

describe("GET /ready", () => {
  it("reports ready only when the complete auth schema contract is present", async () => {
    const response = await fetchHandler(new Request("https://onyx-worker.test/ready"), makeEnv(new MockDb()));
    expect(response.status).toBe(200);
    expect(await response.json()).toMatchObject({ status: "ok", database: "d1", schema: "ok" });
  });

  it("reports the missing schema fields instead of passing SELECT 1", async () => {
    const db = new MockDb();
    db.missingSchemaItems.add("users.password_hash");
    const response = await fetchHandler(new Request("https://onyx-worker.test/ready"), makeEnv(db));
    const body = await response.json() as { missing: string[] };

    expect(response.status).toBe(503);
    expect(body.missing).toContain("users.password_hash");
  });
});

describe("CORS response policy", () => {
  it("echoes an exact allowed origin and never emits wildcard credential CORS", async () => {
    const allowed = await fetchHandler(new Request("https://onyx-worker.test/health", {
      headers: { origin: "https://app.example" },
    }), makeEnv(new MockDb()));
    expect(allowed.headers.get("access-control-allow-origin")).toBe("https://app.example");
    expect(allowed.headers.get("access-control-allow-credentials")).toBe("true");
    expect(allowed.headers.get("vary")).toContain("Origin");

    const denied = await fetchHandler(new Request("https://onyx-worker.test/health", {
      headers: { origin: "https://attacker.example" },
    }), makeEnv(new MockDb()));
    expect(denied.headers.get("access-control-allow-origin")).toBeNull();
    expect(denied.headers.get("access-control-allow-credentials")).toBeNull();
  });
});


describe("POST /api/auth/login — diagnostic correlation and safe failures", () => {
  afterEach(() => vi.restoreAllMocks());

  it("echoes a safe request ID on HTTP credential failures without logging credential material", async () => {
    const db = new MockDb();
    const info = vi.spyOn(console, "log").mockImplementation(() => undefined);
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const username = "diagnostic-user-must-not-be-logged";
    const password = "diagnostic-password-must-not-be-logged";
    const requestId = "login-test-001";
    const response = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        origin: "https://app.example",
        "x-onyx-request-id": requestId,
      },
      body: JSON.stringify({ username, password, client_type: "admin" }),
    }), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(401);
    expect(body).toMatchObject({
      error: "INVALID_CREDENTIALS",
      code: "INVALID_CREDENTIALS",
      correlation_id: requestId,
    });
    expect(response.headers.get("x-onyx-request-id")).toMatch(/^[A-Za-z0-9._:-]{1,80}$/);
    expect(response.headers.get("access-control-allow-origin")).toBe("https://app.example");
    expect(response.headers.get("access-control-expose-headers")).toContain("X-ONYX-Request-ID");

    const logs = JSON.stringify([...info.mock.calls, ...errorLog.mock.calls]);
    expect(logs).toContain(requestId);
    expect(logs).not.toContain(username);
    expect(logs).not.toContain(password);
    expect(logs).not.toContain("access_token");
    expect(logs).not.toContain("refresh_token");
    expect(logs).not.toContain("password_hash");
  });

  it("rejects malformed JSON with an explicit request-validation code", async () => {
    const requestId = "malformed-json-002";
    const response = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json", "x-onyx-request-id": requestId },
      body: "{not-json",
    }), makeEnv(new MockDb()));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(400);
    expect(body).toMatchObject({
      error: "INVALID_LOGIN_REQUEST",
      code: "INVALID_LOGIN_REQUEST",
      correlation_id: requestId,
    });
    expect(response.headers.get("x-onyx-request-id")).toBe(requestId);
  });

  it("rejects non-string credential fields as invalid request schema", async () => {
    const response = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ username: ["not", "a", "string"], password: 42 }),
    }), makeEnv(new MockDb()));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(400);
    expect(body).toMatchObject({ error: "INVALID_LOGIN_REQUEST", code: "INVALID_LOGIN_REQUEST" });
    expect(body.access_token).toBeUndefined();
    expect(body.refresh_token).toBeUndefined();
  });

  it("adds request ID support to CORS preflight for the login endpoint", async () => {
    const response = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "OPTIONS",
      headers: {
        origin: "https://app.example",
        "access-control-request-method": "POST",
        "access-control-request-headers": "content-type,x-onyx-request-id",
      },
    }), makeEnv(new MockDb()));

    expect(response.status).toBe(204);
    expect(response.headers.get("access-control-allow-origin")).toBe("https://app.example");
    expect(response.headers.get("access-control-allow-headers")).toContain("X-ONYX-Request-ID");
    const preflightRequestId = response.headers.get("x-onyx-request-id");
    expect(preflightRequestId).toMatch(/^[A-Za-z0-9._:-]{1,80}$/);

    // A browser preflight carries requested header names, not the future POST's
    // request-ID value. The Worker should generate a valid ID per preflight.
    const secondResponse = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "OPTIONS",
      headers: {
        origin: "https://app.example",
        "access-control-request-method": "POST",
        "access-control-request-headers": "content-type,x-onyx-request-id",
      },
    }), makeEnv(new MockDb()));
    const secondPreflightRequestId = secondResponse.headers.get("x-onyx-request-id");
    expect(secondResponse.status).toBe(204);
    expect(secondPreflightRequestId).toMatch(/^[A-Za-z0-9._:-]{1,80}$/);
    expect(secondPreflightRequestId).not.toBe(preflightRequestId);
  });

  it("records a rejected CORS preflight as server-observed CORS evidence", async () => {
    const info = vi.spyOn(console, "log").mockImplementation(() => undefined);
    const response = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "OPTIONS",
      headers: {
        origin: "https://origin-not-allowed.example",
        "access-control-request-method": "POST",
        "access-control-request-headers": "content-type,x-onyx-request-id",
      },
    }), makeEnv(new MockDb()));

    expect(response.status).toBe(204);
    expect(response.headers.get("access-control-allow-origin")).toBeNull();
    const event = JSON.parse(String(info.mock.calls[0]?.[0])) as Record<string, unknown>;
    expect(event).toMatchObject({
      schema_version: "1.0",
      component: "cloudflare-worker",
      operation: "OPTIONS /api/auth/login",
      stage: "cors",
      correlation_id: expect.stringMatching(/^[A-Za-z0-9._:-]{1,80}$/),
      outcome: "failure",
      error_code: "LOGIN_CORS_ORIGIN_REJECTED",
      cause_confidence: "unknown",
      evidence_source: "cors_policy",
      retryability: "no",
      remediation_ref: "docs/diagnostics/stage-3-unified-diagnostics.md#login-error-mapping",
      event: "onyx.login.http",
      request_id: expect.stringMatching(/^[A-Za-z0-9._:-]{1,80}$/),
      legacy_stage: "cors_preflight",
      legacy_outcome: "cors_rejected",
      legacy_error_code: "LOGIN_CORS_ORIGIN_REJECTED",
      origin: "https://origin-not-allowed.example",
      origin_allowed: false,
      context: {
        route_group: "/api/auth",
        method: "OPTIONS",
        http_status: 204,
        origin_allowed: false,
      },
    });
  });

  it("returns machine-readable method and route errors in the auth namespace", async () => {
    const method = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "GET",
    }), makeEnv(new MockDb()));
    expect(method.status).toBe(405);
    expect(await method.json()).toMatchObject({ error: "METHOD_NOT_ALLOWED" });

    const route = await fetchHandler(new Request("https://onyx-worker.test/api/auth/unknown", {
      method: "POST",
    }), makeEnv(new MockDb()));
    expect(route.status).toBe(404);
    expect(await route.json()).toMatchObject({ error: "AUTH_ROUTE_NOT_FOUND" });
  });

  it("returns a safe session-creation error and logs only allowlisted metadata", async () => {
    const password = "correct horse battery staple";
    const hash = await argon2id({
      password,
      salt: new Uint8Array(16).fill(21),
      parallelism: 1,
      iterations: 1,
      memorySize: 8192,
      hashLength: 32,
      outputType: "encoded",
    });
    const db = new MockDb();
    db.userRow = allfatherRow({ password_hash: hash });
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const requestId = "session-failure-004";
    const response = await fetchHandler(new Request("https://onyx-worker.test/api/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json", "x-onyx-request-id": requestId },
      body: JSON.stringify({ username: ALLFATHER_USERNAME, password, client_type: "admin" }),
    }), { ...makeEnv(db), ONYX_JWT_SECRET: undefined } as unknown as WorkerEnv);
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(500);
    expect(body).toMatchObject({
      error: "LOGIN_SESSION_CREATION_FAILED",
      code: "LOGIN_SESSION_CREATION_FAILED",
      correlation_id: requestId,
    });
    expect(body.access_token).toBeUndefined();
    expect(body.refresh_token).toBeUndefined();

    const logEvent = JSON.parse(String(errorLog.mock.calls[0]?.[0])) as Record<string, unknown>;
    expect(logEvent).toMatchObject({
      schema_version: "1.0",
      component: "cloudflare-worker",
      operation: "POST /api/auth/login",
      stage: "session",
      correlation_id: requestId,
      outcome: "failure",
      error_code: "LOGIN_SESSION_CREATION_FAILED",
      cause_confidence: "unknown",
      evidence_source: "server_code",
      retryability: "yes",
      legacy_stage: "session_creation",
      diagnostic_code: "LOGIN_SESSION_CREATION_FAILED",
      context: {
        route_group: "/api/auth",
        method: "POST",
        http_status: 500,
      },
    });
    const logs = JSON.stringify(errorLog.mock.calls);
    expect(logs).toContain(requestId);
    expect(logs).toContain("LOGIN_SESSION_CREATION_FAILED");
    expect(logs).not.toContain(password);
    expect(logs).not.toContain("password_hash");
    expect(logs).not.toContain("ONYX_JWT_SECRET is not configured");
  });
});
