/**
 * Phase 1 regression tests for the ONYX Worker authentication trust boundary.
 * These tests exercise the real Worker handler with a D1 test double; they do
 * not claim to verify deployed Cloudflare or production D1 behavior.
 */
import { describe, expect, it } from "vitest";
import { argon2id } from "hash-wasm";
import worker from "../src/index";
import { AUTH_SCHEMA_CONTRACT } from "../src/schema-contract";

type WorkerEnv = Parameters<typeof worker.fetch>[1];

class AuthDb {
  userRow: Record<string, unknown> | null = null;
  prepare(sql: string) {
    const tablePrefix = "PRAGMA table_info(";
    const indexPrefix = "pragmaindex_list(";
    const rows = (): Array<Record<string, unknown>> => {
      if (sql.toLowerCase().startsWith(tablePrefix.toLowerCase()) && sql.endsWith(")")) {
        const table = sql.slice(tablePrefix.length, -1);
        return (AUTH_SCHEMA_CONTRACT.tables[table] ?? []).map((name) => ({ name }));
      }
      const compact = sql.toLowerCase().replaceAll(" ", "");
      if (compact.startsWith(indexPrefix) && compact.endsWith(")")) {
        const table = compact.slice(indexPrefix.length, -1);
        const indexes = table === "users" ? AUTH_SCHEMA_CONTRACT.indexes : AUTH_SCHEMA_CONTRACT.tableIndexes[table] ?? [];
        return indexes.map((name) => ({ name }));
      }
      return [];
    };
    const first = async () => sql.includes("FROM users") ? this.userRow : null;
    return {
      all: async () => ({ results: rows() }),
      first,
      bind: (..._params: unknown[]) => ({
        first,
        all: async () => ({ results: rows() }),
        run: async () => ({ success: true, meta: { changes: 1 } }),
      }),
    };
  }
}

function env(db: AuthDb): WorkerEnv {
  return {
    DB: db,
    ONYX_JWT_SECRET: "phase-1-test-secret",
    ONYX_CORS_ORIGINS: "https://app.example",
  } as unknown as WorkerEnv;
}

async function hashFor(password: string): Promise<string> {
  return String(await argon2id({
    password,
    salt: new Uint8Array(16).fill(31),
    parallelism: 1,
    iterations: 1,
    memorySize: 8192,
    hashLength: 32,
    outputType: "encoded",
  }));
}

function user(overrides: Record<string, unknown> = {}) {
  return {
    id: "staff-1",
    username: "staff1",
    email: "staff1@example.test",
    organization_id: "org-1",
    password_hash: "",
    is_admin: 0,
    is_active: 1,
    class: "STAFF",
    role: "STAFF",
    ...overrides,
  };
}

async function login(db: AuthDb, body: Record<string, unknown>) {
  const response = await worker.fetch(new Request("https://onyx-worker.test/api/auth/login", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  }), env(db));
  return { response, body: await response.json() as Record<string, unknown> };
}

describe("Phase 1 auth trust-boundary regressions", () => {
  it.each(["", "not-a-hash", "$argon2id$malformed"])(
    "fails closed for a missing, unsupported, or malformed password hash (%s)",
    async (password_hash) => {
      const db = new AuthDb();
      db.userRow = user({ password_hash });
      const result = await login(db, { username: "staff1", password: "any-password" });
      expect(result.response.status).toBe(401);
      expect(result.body.error).toBe("INVALID_CREDENTIALS");
      expect(result.body.access_token).toBeUndefined();
      expect(result.body.refresh_token).toBeUndefined();
    },
  );

  it("does not allow client metadata to elevate Staff into All-Father or Admin", async () => {
    const password = "correct horse battery staple";
    const db = new AuthDb();
    db.userRow = user({ password_hash: await hashFor(password) });
    const result = await login(db, {
      username: "staff1",
      password,
      id: "__allfather__",
      sub: "__allfather__",
      role: "ALL_FATHER",
      is_admin: true,
      organization_id: "__allfather__",
      allfather: true,
      client_type: "admin",
    });

    expect(result.response.status).toBe(200);
    expect(result.body.access_token).toEqual(expect.any(String));
    const issuedUser = result.body.user as Record<string, unknown>;
    expect(issuedUser.id).toBe("staff-1");
    expect(issuedUser.role).toBe("STAFF");
    expect(issuedUser.is_admin).toBe(false);
    expect(issuedUser.organization_id).toBe("org-1");
  });

  it("rejects ALL_FATHER role on a non-canonical principal even with a valid password and admin flag", async () => {
    const password = "correct horse battery staple";
    const db = new AuthDb();
    db.userRow = user({
      id: "ordinary-user",
      username: "allfather",
      role: "ALL_FATHER",
      is_admin: 1,
      password_hash: await hashFor(password),
    });
    const result = await login(db, { username: "allfather", password });
    expect(result.response.status).toBe(401);
    expect(result.body.error).toBe("INVALID_CREDENTIALS");
    expect(result.body.access_token).toBeUndefined();
  });
});
