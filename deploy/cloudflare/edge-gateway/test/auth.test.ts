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

  prepare(sql: string) {
    const record = (params: unknown[]) => this.queries.push({ sql, params });
    const row = () => (sql.includes("FROM users") ? this.userRow : null);
    return {
      bind: (...params: unknown[]) => {
        record(params);
        return {
          first: async () => row(),
          run: async () => ({ success: true }),
          all: async () => ({ results: [] }),
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
    SUPABASE_URL,
    SUPABASE_PUBLISHABLE_KEY: "pk_test_dummy",
  } as unknown as WorkerEnv;
}

function allfatherRow(overrides: Record<string, unknown> = {}) {
  return {
    id: "__onyx_allfather__",
    username: "allfather",
    email: ALLFATHER_EMAIL,
    supabase_user_id: SUPABASE_SUB,
    organization_id: "__onyx_root__",
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
