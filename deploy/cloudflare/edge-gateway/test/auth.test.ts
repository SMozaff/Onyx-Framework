/**
 * Authentication contract tests for ONYX username/password login.
 * Google/Supabase is not an All-Father authentication path.
 */
import { describe, expect, it } from "vitest";
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
    expect(wrongBody).toEqual(body);
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
