import { describe, expect, it } from "vitest";
import worker from "../src/index";

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
  return btoa(binary).replace(/\\+/g, "-").replace(/\\//g, "_").replace(/=+$/g, "");
}
async function signAccessToken(): Promise<string> {
  const payload = {
    iss: "onyx-cloudflare-worker",
    aud: "onyx-api",
    sub: USER,
    username: "staff",
    organization_id: ORG,
    is_admin: false,
    class: "STAFF",
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
class RelayDb {
  owners = new Map<string, { user_id: string; organization_id: string }>();
  redeemed = new Set<string>();

  prepare(sql: string) {
    let params: unknown[] = [];
    const db = this;
    const statement: any = {
      bind(...values: unknown[]) { params = values; return statement; },
      first: async () => {
        if (sql.includes("FROM users u JOIN organizations")) {
          if (String(params[0]) !== USER) return null;
          return {
            id: USER, username: "staff", email: "staff@example.test",
            organization_id: ORG, is_active: 1, is_admin: 0,
            class: "STAFF", role: "STAFF", supabase_user_id: null,
          };
        }
        if (sql.includes("FROM user_session_revocations")) return null;
        if (sql.includes("FROM replica_ownership WHERE replica_id=?")) {
          const owner = db.owners.get(String(params[0]).toLowerCase());
          return owner ? { ...owner } : null;
        }
        return null;
      },
      run: async () => {
        if (sql.includes("INSERT INTO replica_ownership")) {
          const replica = String(params[0]).toLowerCase();
          if (!db.owners.has(replica)) {
            db.owners.set(replica, { user_id: String(params[1]), organization_id: String(params[2]) });
          }
          return { success: true, meta: { changes: 1 } };
        }
        if (sql.includes("INSERT OR IGNORE INTO relay_ticket_redemptions")) {
          const jti = String(params[0]);
          if (db.redeemed.has(jti)) return { success: true, meta: { changes: 0 } };
          db.redeemed.add(jti);
          return { success: true, meta: { changes: 1 } };
        }
        return { success: true, meta: { changes: 1 } };
      },
    };
    return statement;
  }
}

describe("Worker Cloud Relay route integration", () => {
  it("mints a scoped ticket, forwards the WebSocket route to RelayHub, and rejects ticket replay", async () => {
    const db = new RelayDb();
    let forwarded: Request | undefined;
    let routedOrganization: string | undefined;
    const env = {
      DB: db,
      ONYX_JWT_SECRET: SECRET,
      ONYX_CORS_ORIGINS: "",
      RELAY_HUB: {
        idFromName(name: string) { routedOrganization = name; return name; },
        get(_id: unknown) {
          return {
            async fetch(request: Request) {
              forwarded = request;
              // The test exercises the real Worker route and auth/ticket
              // checks. A local D1/DO runtime is needed for an actual 101
              // upgrade handshake, so the namespace boundary is stubbed here.
              return new Response("relay hub reached", { status: 200 });
            },
          };
        },
      },
    } as unknown as WorkerEnv;

    const accessToken = await signAccessToken();
    const ticketResponse = await worker.fetch(new Request("https://worker.test/api/relay-ticket", {
      method: "POST",
      headers: { authorization: `Bearer ${accessToken}`, "content-type": "application/json" },
      body: JSON.stringify({ target_id: TARGET, self_replica: SELF }),
    }), env);
    expect(ticketResponse.status).toBe(200);
    const ticketBody = await ticketResponse.json() as { ticket?: string };
    expect(typeof ticketBody.ticket).toBe("string");
    expect(ticketBody.ticket!.length).toBeGreaterThan(20);

    const socketRequest = new Request(`https://worker.test/api/relay/${TARGET}?ticket=${encodeURIComponent(ticketBody.ticket!)}&self=${SELF}`);
    // Node's fetch Request implementation does not permit constructing a
    // browser-style Upgrade request. Override only the request-header read
    // observed by the Worker route; the production Worker runtime supplies it.
    const originalGet = socketRequest.headers.get.bind(socketRequest.headers);
    socketRequest.headers.get = ((name: string) =>
      name.toLowerCase() === "upgrade" ? "websocket" : originalGet(name)) as typeof socketRequest.headers.get;

    const relayResponse = await worker.fetch(socketRequest, env);
    expect(relayResponse.status).toBe(200);
    expect(await relayResponse.text()).toBe("relay hub reached");
    expect(routedOrganization).toBe(ORG);
    expect(forwarded?.url).toContain(`/api/relay/${TARGET}`);
    expect(forwarded?.headers.get("x-onyx-relay-org")).toBe(ORG);
    expect(forwarded?.headers.get("x-onyx-relay-user")).toBe(USER);
    expect(forwarded?.headers.get("x-onyx-relay-self")).toBe(SELF);
    expect(forwarded?.headers.get("x-onyx-relay-target")).toBe(TARGET);

    const replay = new Request(`https://worker.test/api/relay/${TARGET}?ticket=${encodeURIComponent(ticketBody.ticket!)}&self=${SELF}`);
    replay.headers.get = ((name: string) =>
      name.toLowerCase() === "upgrade" ? "websocket" : originalGet(name)) as typeof replay.headers.get;
    const replayResponse = await worker.fetch(replay, env);
    expect(replayResponse.status).toBe(401);
    expect(await replayResponse.json()).toMatchObject({ error: "RELAY_TICKET_REPLAYED" });
  });
});
