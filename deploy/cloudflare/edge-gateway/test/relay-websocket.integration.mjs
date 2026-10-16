import { createHmac, randomUUID } from "node:crypto";

const baseUrl = process.env.ONYX_TEST_WORKER_URL ?? "http://127.0.0.1:8787";
const secret = process.env.ONYX_JWT_SECRET ?? "relay-test-secret";
const organizationId = "11111111-2222-4333-8444-555555555555";
const userId = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee";
const selfReplica = "12345678-1234-4234-8234-123456789abc";
const targetReplica = "87654321-4321-4321-8321-cba987654321";

function signAccessToken() {
  const now = Math.floor(Date.now() / 1000);
  const header = Buffer.from(JSON.stringify({ alg: "HS256", typ: "JWT" })).toString("base64url");
  const payload = Buffer.from(JSON.stringify({
    iss: "onyx-cloudflare-worker",
    aud: "onyx-api",
    sub: userId,
    username: "relay-test-staff",
    organization_id: organizationId,
    is_admin: false,
    class: "staff",
    role: "STAFF",
    client_type: "desktop",
    exp: now + 300,
    iat_ms: Date.now(),
    jti: randomUUID(),
    sid: "relay-websocket-integration",
    typ: "access",
  })).toString("base64url");
  const input = `${header}.${payload}`;
  const signature = createHmac("sha256", secret).update(input).digest("base64url");
  return `${input}.${signature}`;
}

const accessToken = signAccessToken();
const ticketResponse = await fetch(new URL("/api/relay-ticket", baseUrl), {
  method: "POST",
  headers: {
    authorization: `Bearer ${accessToken}`,
    "content-type": "application/json",
  },
  body: JSON.stringify({ target_id: targetReplica, self_replica: selfReplica }),
});
if (!ticketResponse.ok) {
  throw new Error(`Relay ticket issuance failed: HTTP ${ticketResponse.status} ${await ticketResponse.text()}`);
}
const { ticket } = await ticketResponse.json();
if (typeof ticket !== "string" || ticket.length < 20) {
  throw new Error("Worker returned an invalid relay ticket");
}

const websocketUrl = new URL(`/api/relay/${targetReplica}`, baseUrl);
websocketUrl.protocol = websocketUrl.protocol === "https:" ? "wss:" : "ws:";
websocketUrl.searchParams.set("ticket", ticket);
websocketUrl.searchParams.set("self", selfReplica);

const socket = new WebSocket(websocketUrl);
let opened = false;
try {
  await new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("Relay WebSocket handshake timed out")), 10_000);
    socket.addEventListener("open", () => {
      opened = true;
      clearTimeout(timeout);
      resolve();
    }, { once: true });
    socket.addEventListener("error", () => {
      clearTimeout(timeout);
      reject(new Error("Relay WebSocket handshake failed"));
    }, { once: true });
    socket.addEventListener("close", (event) => {
      if (!opened) {
        clearTimeout(timeout);
        reject(new Error(`Relay closed before opening: code=${event.code} reason=${event.reason}`));
      }
    }, { once: true });
  });
  console.log("PASS: Worker issued a scoped ticket and accepted a real RelayHub WebSocket upgrade.");
} finally {
  socket.close();
}
