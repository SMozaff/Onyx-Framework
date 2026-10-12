/**
 * Focused authentication tests for the ONYX Cloudflare Worker.
 *
 * Phase 2 (reports/auth-repair/02-auth-contract.md) — negative authorization
 * cases for the All-Father Supabase exchange and the ONYX password path:
 *  - AUTH-01: login must never create or replace users.supabase_user_id.
 *  - AUTH-04: public auth failures return stable error codes only.
 *
 * No real network calls: Supabase JWKS and Auth user responses are mocked;
 * JWT signatures are real ES256 signatures over a locally generated key.
 */
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";
import { argon2id } from "hash-wasm";
import worker from "../src/index";
import { AUTH_SCHEMA_CONTRACT } from "../src/schema-contract";

const SUPABASE_URL = "https://example-project.supabase.co";
const ALLFATHER_EMAIL = "so.muzaff@gmail.com";
const SUPABASE_SUB = "11111111-2222-3333-4444-555555555555";
const EXPECTED_ISSUER = `${SUPABASE_URL}/auth/v1`;

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
    ONYX_ALLFATHER_EMAIL: ALLFATHER_EMAIL,
    ONYX_CORS_ORIGINS: "https://app.example",
    SUPABASE_URL,
    SUPABASE_PUBLISHABLE_KEY: "pk_test_dummy",
  } as unknown as WorkerEnv;
}

function allfatherRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "__allfather__",
    username: "allfather",
    email: ALLFATHER_EMAIL,
    supabase_user_id: SUPABASE_SUB,
    organization_id: "__allfather__",
    is_admin: 1,
    is_active: 1,
    class: "ALL_FATHER",
    role: "ALL_FATHER",
    ...overrides,
  };
}

// --- Supabase JWT signing with a locally generated ES256 key ---------------

let signingKey: CryptoKey;
let publicJwk: Record<string, unknown>;

beforeAll(async () => {
  const pair = (await crypto.subtle.generateKey(
    { name: "ECDSA", namedCurve: "P-256" },
    true,
    ["sign", "verify"],
  )) as CryptoKeyPair;
  signingKey = pair.privateKey;
  publicJwk = {
    ...(await crypto.subtle.exportKey("jwk", pair.publicKey)),
    kid: "test-key",
    alg: "ES256",
  };
});

function b64u(value: unknown): string {
  return Buffer.from(JSON.stringify(value)).toString("base64url");
}

async function signSupabaseJwt(
  payload: Record<string, unknown>,
  options: { tamperSignature?: boolean } = {},
): Promise<string> {
  const input = `${b64u({ alg: "ES256", kid: "test-key" })}.${b64u(payload)}`;
  const signature = await crypto.subtle.sign(
    { name: "ECDSA", hash: "SHA-256" },
    signingKey,
    new TextEncoder().encode(input),
  );
  let encoded = Buffer.from(signature).toString("base64url");
  if (options.tamperSignature) {
    encoded = (encoded[0] === "A" ? "B" : "A") + encoded.slice(1);
  }
  return `${input}.${encoded}`;
}

// --- Supabase endpoint mock ----------------------------------------------

let identityBody: Record<string, unknown> = {};
const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
  const url = String(input);
  if (url.includes("/.well-known/jwks.json")) return Response.json({ keys: [publicJwk] });
  if (url.endsWith("/auth/v1/user")) return Response.json(identityBody);
  throw new Error(`unexpected outbound fetch: ${url}`);
});

beforeEach(() => {
  vi.stubGlobal("fetch", fetchMock);
  identityBody = {
    id: SUPABASE_SUB,
    email: ALLFATHER_EMAIL,
    email_confirmed_at: "2026-01-01T00:00:00.000Z",
  };
});

function supabaseRequest(token: string): Request {
  return new Request("https://onyx-worker.test/api/auth/supabase", {
    method: "POST",
    headers: { authorization: `Bearer ${token}` },
  });
}

function validClaims(overrides: Record<string, unknown> = {}) {
  return {
    sub: SUPABASE_SUB,
    aud: "authenticated",
    email: ALLFATHER_EMAIL,
    email_verified: true,
    iss: EXPECTED_ISSUER,
    exp: Math.floor(Date.now() / 1000) + 300,
    ...overrides,
  };
}

describe("POST /api/auth/supabase (All-Father exchange)", () => {
  it("issues an ONYX session when the Supabase subject is explicitly provisioned", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    const token = await signSupabaseJwt(validClaims());

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(200);
    expect(body.access_token).toEqual(expect.any(String));
    expect(body.refresh_token).toEqual(expect.any(String));
    expect(body.expires_in).toBe(3600);
    expect((body.user as Record<string, unknown>).username).toBe("allfather");
    expect(db.queries.some((query) => query.sql.includes("WHERE u.id=?") && query.params.includes("__allfather__"))).toBe(true);
    expect(db.writes()).toHaveLength(0);
  });

  it("AUTH-01: denies an unmapped subject without any database write", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow({ supabase_user_id: null });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const token = await signSupabaseJwt(validClaims());

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(403);
    expect(body.error).toBe("ALLFATHER_NOT_PROVISIONED");
    expect(db.writes()).toHaveLength(0);
    expect(JSON.stringify(warn.mock.calls)).toContain("SUBJECT_UNMAPPED");
    warn.mockRestore();
  });

  it("AUTH-01: denies a replaced subject without rewriting the mapping", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow({ supabase_user_id: "an-earlier-subject" });
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const token = await signSupabaseJwt(validClaims());

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(403);
    expect(body.error).toBe("ALLFATHER_NOT_PROVISIONED");
    expect(db.writes()).toHaveLength(0);
    expect(db.queries.some((q) => q.sql.includes("UPDATE users SET supabase_user_id"))).toBe(false);
    expect(JSON.stringify(warn.mock.calls)).toContain("SUBJECT_MISMATCH");
    warn.mockRestore();
  });

  it("AUTH-04: returns a stable generic error for an invalid signature", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => {});
    const token = await signSupabaseJwt(validClaims(), { tamperSignature: true });

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const raw = await response.text();

    expect(response.status).toBe(401);
    const body = JSON.parse(raw) as Record<string, unknown>;
    expect(body.error).toBe("INVALID_SUPABASE_TOKEN");
    expect(Object.keys(body).sort()).toEqual(["category", "error", "retryability"]);
    expect(raw).not.toMatch(/signature|jwks|claims|unsupported/i);
    expect(raw).not.toContain(token);
    expect(errorLog).toHaveBeenCalled();
    expect(JSON.stringify(errorLog.mock.calls)).not.toContain(token);
    errorLog.mockRestore();
  });

  it("rejects a Supabase user whose id differs from the JWT subject", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    identityBody = { ...identityBody, id: "not-the-jwt-subject" };
    const token = await signSupabaseJwt(validClaims());

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(403);
    expect(body.error).toBe("ALLFATHER_REQUIRED");
    expect(db.writes()).toHaveLength(0);
  });

  it("rejects an unconfirmed Supabase email", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    identityBody = { ...identityBody, email_confirmed_at: null };
    const token = await signSupabaseJwt(validClaims());

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(403);
    expect(body.error).toBe("ALLFATHER_REQUIRED");
    expect(db.writes()).toHaveLength(0);
  });

  it("rejects a token issued for the wrong issuer", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    const token = await signSupabaseJwt(validClaims({ iss: "https://evil.example/auth/v1" }));

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(401);
    expect(body.error).toBe("INVALID_SUPABASE_TOKEN");
    expect(db.writes()).toHaveLength(0);
  });

  it("rejects a token whose audience is not the Supabase authenticated audience", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => {});
    const token = await signSupabaseJwt(validClaims({ aud: "anon" }));

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(401);
    expect(body.error).toBe("INVALID_SUPABASE_TOKEN");
    expect(db.writes()).toHaveLength(0);
    errorLog.mockRestore();
  });

  it("rejects a token with no audience claim", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => {});
    const claims = validClaims();
    delete (claims as Record<string, unknown>).aud;
    const token = await signSupabaseJwt(claims);

    const response = await fetchHandler(supabaseRequest(token), makeEnv(db));
    expect(response.status).toBe(401);
    expect(db.writes()).toHaveLength(0);
    errorLog.mockRestore();
  });

  it("accepts an array audience that includes authenticated and rejects a future nbf", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    const arrayAud = await signSupabaseJwt(validClaims({ aud: ["authenticated"] }));
    const okResponse = await fetchHandler(supabaseRequest(arrayAud), makeEnv(db));
    expect(okResponse.status).toBe(200);

    const errorLog = vi.spyOn(console, "error").mockImplementation(() => {});
    const futureNbf = await signSupabaseJwt(
      validClaims({ nbf: Math.floor(Date.now() / 1000) + 3600 }),
    );
    const notYetValid = await fetchHandler(supabaseRequest(futureNbf), makeEnv(db));
    expect(notYetValid.status).toBe(401);
    expect(db.writes()).toHaveLength(0);
    errorLog.mockRestore();
  });
});

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



describe("protected routes and token kinds", () => {
  it("accepts ONYX access tokens but rejects refresh tokens as bearer credentials", async () => {
    const db = new MockDb();
    db.userRow = allfatherRow();
    const providerToken = await signSupabaseJwt(validClaims());
    const exchange = await fetchHandler(supabaseRequest(providerToken), makeEnv(db));
    const session = await exchange.json() as { access_token: string; refresh_token: string };

    const accessResponse = await fetchHandler(new Request("https://onyx-worker.test/api/profiles", {
      headers: { authorization: `Bearer ${session.access_token}` },
    }), makeEnv(db));
    const refreshResponse = await fetchHandler(new Request("https://onyx-worker.test/api/profiles", {
      headers: { authorization: `Bearer ${session.refresh_token}` },
    }), makeEnv(db));

    expect(accessResponse.status).toBe(200);
    expect(refreshResponse.status).toBe(401);
    expect(await refreshResponse.json()).toMatchObject({ error: "UNAUTHORIZED" });
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
    db.missingSchemaItems.add("users.supabase_user_id");
    const response = await fetchHandler(new Request("https://onyx-worker.test/ready"), makeEnv(db));
    const body = await response.json() as { missing: string[] };

    expect(response.status).toBe(503);
    expect(body.missing).toContain("users.supabase_user_id");
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
