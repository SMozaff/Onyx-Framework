import { describe, expect, it } from "vitest";
import worker from "../src/index";
import { AUTH_SCHEMA_CONTRACT } from "../src/schema-contract";

const SECRET = "relay-route-test-secret";
const ORG = "11111111-2222-4333-8444-555555555555";
const USER = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
const SELF = "12345678-1234-4234-8234-123456789abc";
const TARGET = "87654321-4321-4321-8321-cba987654321";
type WorkerEnv = Parameters<typeof worker.fetch>[1];

const encoder = new TextEncoder();
function b64u(input: string | Uint8Array): string {
  const bytes = typeof input === "string" ? encoder.encode(input) : input;
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}
async function signAccessToken(): Promise<string> {
  const payload = {
    iss: "onyx-cloudflare-worker",
    aud: "onyx-api",
    sub: USER,
    username: "staff",
    organization_id: ORG,
    is_admin: false,
    class: "staff",
    role: "STAFF",
    client_type: "desktop",
    exp: Math.floor(Date.now() / 1000) + 3600,
    iat_ms: Date.now(),
    jti: crypto.randomUUID(),
    sid: "relay-test-session",
    typ: "access",
  };
  const input = `${b64u(JSON.stringify({ alg: "HS256", typ: "JWT" }))}.${b64u(JSON.stringify(payload))}`;
  const key = await crypto.subtle.importKey("raw", encoder.encode(SECRET), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const signature = new Uint8Array(await crypto.subtle.sign("HMAC", key, encoder.encode(input)));
  return `${input}.${b64u(signature)}`;
}

/** Minimal D1 test double for the actual Worker ticket + relay route handlers. */

/** D1 test double for the outbox ingest route: auth lookups plus outbox_ingest writes. */
class IngestDb {
  owners = new Map<string, { user_id: string; organization_id: string }>();
  rows = new Map<string, Record<string, unknown>>();
  failBatch = false;
  batchCalls = 0;

  prepare(sql: string) {
    let params: unknown[] = [];
    const db = this;
    const statement: any = {
      all: async () => {
        const tablePrefix = "PRAGMA table_info(";
        if (sql.toLowerCase().startsWith(tablePrefix.toLowerCase()) && sql.endsWith(")")) {
          const table = sql.slice(tablePrefix.length, -1);
          return { results: (AUTH_SCHEMA_CONTRACT.tables[table] ?? []).map((name) => ({ name })) };
        }
        const compact = sql.toLowerCase().replaceAll(" ", "");
        const indexPrefix = "pragmaindex_list(";
        if (compact.startsWith(indexPrefix) && compact.endsWith(")")) {
          const table = compact.slice(indexPrefix.length, -1);
          const indexes = table === "users" ? AUTH_SCHEMA_CONTRACT.indexes : AUTH_SCHEMA_CONTRACT.tableIndexes[table] ?? [];
          return { results: indexes.map((name) => ({ name })) };
        }
        return { results: [] };
      },
      bind(...values: unknown[]) { params = values; return statement; },
      first: async () => {
        if (sql.includes("SELECT class FROM users")) return { class: "staff" };
        if (sql.includes("FROM users u JOIN organizations")) {
          if (String(params[0]) !== USER) return null;
          return {
            id: USER, username: "staff", email: "staff@example.test",
            organization_id: ORG, is_active: 1, is_admin: 0,
            class: "staff", role: "STAFF", supabase_user_id: null,
          };
        }
        if (sql.includes("FROM user_session_revocations")) return null;
        if (sql.includes("FROM token_revocations")) return null;
        if (sql.includes("FROM replica_ownership WHERE replica_id=?")) {
          const owner = db.owners.get(String(params[0]).toLowerCase());
          return owner ? { ...owner } : null;
        }
        return null;
      },
      run: async () => {
        if (sql.includes("INSERT OR IGNORE INTO outbox_ingest")) {
          const key = `${params[0]}:${params[1]}`;
          if (db.rows.has(key)) return { success: true, meta: { changes: 0 } };
          db.rows.set(key, {
            replica_id: params[0], outbox_id: params[1], organization_id: params[2],
            user_id: params[3], event_id: params[4], event_type: params[5],
            aggregate_id: params[6], occurred_at_millis: params[7], payload: params[8],
          });
          return { success: true, meta: { changes: 1 } };
        }
        return { success: true, meta: { changes: 0 } };
      },
    };
    return statement;
  }

  async batch(statements: Array<{ run: () => Promise<unknown> }>) {
    this.batchCalls += 1;
    if (this.failBatch) throw new Error("D1 unavailable");
    const results = [];
    for (const s of statements) results.push(await s.run());
    return results;
  }
}

const REPLICA = "12345678-1234-4234-8234-123456789abc";
const EVENT_A = "0f0f0f0f-0f0f-4f0f-8f0f-0f0f0f0f0f0f";
const EVENT_B = "1a1a1a1a-1a1a-4a1a-8a1a-1a1a1a1a1a1a";

function item(outboxId: number, overrides: Record<string, unknown> = {}) {
  return {
    outbox_id: outboxId,
    event_id: outboxId === 1 ? EVENT_A : EVENT_B,
    event_type: "TestEvent",
    aggregate_id: "agg-1",
    organization_id: ORG,
    occurred_at_millis: 1_700_000_000_000,
    payload: { n: outboxId },
    ...overrides,
  };
}

function ingestEnv(db: IngestDb) {
  return { DB: db, ONYX_JWT_SECRET: SECRET, ONYX_CORS_ORIGINS: "" } as unknown as WorkerEnv;
}

async function post(env: WorkerEnv, token: string, body: unknown) {
  return worker.fetch(new Request("https://worker.test/api/sync/outbox", {
    method: "POST",
    headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: typeof body === "string" ? body : JSON.stringify(body),
  }), env);
}

describe("Worker outbox ingest route", () => {
  it("stores a batch, acks each row, and treats a re-send as stored without duplicating", async () => {
    const db = new IngestDb();
    db.owners.set(REPLICA, { user_id: USER, organization_id: ORG });
    const env = ingestEnv(db);
    const token = await signAccessToken();

    const first = await post(env, token, { replica_id: REPLICA, items: [item(1), item(2)] });
    expect(first.status, await first.clone().text()).toBe(200);
    expect(await first.json()).toEqual({
      acks: [
        { outbox_id: 1, status: "stored" },
        { outbox_id: 2, status: "stored" },
      ],
    });
    expect(db.rows.size).toBe(2);
    expect(db.rows.get(`${REPLICA}:1`)?.payload).toBe(JSON.stringify({ n: 1 }));

    const resend = await post(env, token, { replica_id: REPLICA, items: [item(1)] });
    expect(await resend.json()).toEqual({ acks: [{ outbox_id: 1, status: "stored" }] });
    expect(db.rows.size).toBe(2);
  });

  it("refuses a replica the authenticated user does not own", async () => {
    const db = new IngestDb();
    db.owners.set(REPLICA, { user_id: "someone-else-0000-4000-8000-000000000000", organization_id: ORG });
    const response = await post(ingestEnv(db), await signAccessToken(), { replica_id: REPLICA, items: [item(1)] });
    expect(response.status).toBe(401);
    expect(await response.json()).toMatchObject({ error: "REPLICA_OWNERSHIP_REJECTED" });
    expect(db.rows.size).toBe(0);
  });

  it("rejects a row from another organization without storing it, and stores the rest", async () => {
    const db = new IngestDb();
    db.owners.set(REPLICA, { user_id: USER, organization_id: ORG });
    const response = await post(ingestEnv(db), await signAccessToken(), {
      replica_id: REPLICA,
      items: [item(1, { organization_id: "99999999-9999-4999-8999-999999999999" }), item(2)],
    });
    expect(await response.json()).toEqual({
      acks: [
        { outbox_id: 1, status: "rejected", retryable: false, reason: "organization mismatch" },
        { outbox_id: 2, status: "stored" },
      ],
    });
    expect(db.rows.has(`${REPLICA}:1`)).toBe(false);
    expect(db.rows.has(`${REPLICA}:2`)).toBe(true);
  });

  it("returns a retryable 503 and acks nothing when the D1 write fails", async () => {
    const db = new IngestDb();
    db.owners.set(REPLICA, { user_id: USER, organization_id: ORG });
    db.failBatch = true;
    const response = await post(ingestEnv(db), await signAccessToken(), { replica_id: REPLICA, items: [item(1)] });
    expect(response.status).toBe(503);
    expect(await response.json()).toMatchObject({ error: "OUTBOX_STORE_UNAVAILABLE", retryability: "RETRYABLE" });
    expect(db.rows.size).toBe(0);
  });

  it("rejects malformed batches before touching storage", async () => {
    const db = new IngestDb();
    db.owners.set(REPLICA, { user_id: USER, organization_id: ORG });
    const env = ingestEnv(db);
    const token = await signAccessToken();
    expect((await post(env, token, "not json")).status).toBe(400);
    expect((await post(env, token, { replica_id: "nope", items: [item(1)] })).status).toBe(400);
    expect((await post(env, token, { replica_id: REPLICA, items: [] })).status).toBe(400);
    expect((await post(env, token, { replica_id: REPLICA, items: [{ outbox_id: -1 }] })).status).toBe(400);
    expect(db.batchCalls).toBe(0);
  });
});
