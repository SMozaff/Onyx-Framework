#!/usr/bin/env node
/**
 * End-to-end All-Father auth smoke against a local Wrangler Worker + local D1.
 * It refuses non-loopback targets, seeds a disposable ordinary user, never
 * logs credentials/tokens, and removes the disposable user/organization.
 */
import { argon2id, argon2Verify } from "hash-wasm";
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const workerRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const DATABASE_NAME = "onyx-free-db";
const CONFIG = "wrangler.dev.toml";

function wranglerEnvironment() {
  const env = { ...process.env };
  delete env.ONYX_ALLFATHER_USERNAME;
  delete env.ONYX_ALLFATHER_PASSWORD;
  return env;
}

function fail(message) {
  console.error(`::error::${message}`);
  process.exitCode = 1;
  throw new Error(message);
}
function quote(value) { return "'" + String(value).replace(/'/g, "''") + "'"; }
function decodeClaims(token) {
  const part = token.split(".")[1];
  if (!part) return null;
  try { return JSON.parse(Buffer.from(part.replace(/-/g, "+").replace(/_/g, "/"), "base64").toString("utf8")); }
  catch { return null; }
}
function localSql(sql) {
  const dir = mkdtempSync(path.join(tmpdir(), "onyx-auth-smoke-"));
  const file = path.join(dir, "operation.sql");
  try {
    writeFileSync(file, sql, { mode: 0o600 });
    const output = execFileSync("npx", [
      "wrangler", "d1", "execute", DATABASE_NAME, "--local", "--config", CONFIG, "--file", file, "--json",
    ], { cwd: workerRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: wranglerEnvironment() });
    const parsed = JSON.parse(output);
    const list = Array.isArray(parsed) ? parsed : [parsed];
    return list.find((item) => Array.isArray(item?.results))?.results ?? [];
  } catch {
    fail("Local D1 fixture operation failed; SQL and credential material suppressed.");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function main() {
  const username = process.env.ONYX_ALLFATHER_USERNAME ?? "";
  const password = process.env.ONYX_ALLFATHER_PASSWORD ?? "";
  const baseRaw = process.env.ONYX_AUTH_SMOKE_BASE_URL ?? "";
  if (username.trim().toLowerCase() !== "allfather" || !password) {
    fail("Set ONYX_ALLFATHER_USERNAME to the reserved username and provide ONYX_ALLFATHER_PASSWORD.");
  }
  let base;
  try { base = new URL(baseRaw); } catch { fail("ONYX_AUTH_SMOKE_BASE_URL must be a valid local Worker URL."); }
  if (!["127.0.0.1", "localhost", "::1"].includes(base.hostname) || !["http:", "https:"].includes(base.protocol)) {
    fail("Smoke tests may target loopback only; production and remote hosts are forbidden.");
  }
  base = base.toString().replace(/\/+$/, "");

  const userId = randomUUID();
  const orgId = randomUUID();
  const staffUsername = `auth-smoke-${randomUUID().slice(0, 8)}`;
  const staffPassword = randomBytes(32).toString("base64url");
  const staffHash = String(await argon2id({
    password: staffPassword, salt: randomBytes(16), parallelism: 1,
    iterations: 3, memorySize: 65536, hashLength: 32, outputType: "encoded",
  }));
  const now = Math.floor(Date.now() / 1000);
  let fixtureMayExist = false;
  let consumedRefreshHash = null;
  try {
    fixtureMayExist = true;
    localSql(`INSERT INTO organizations(id,name,is_active,created_at,updated_at)
VALUES(${quote(orgId)},'All-Father smoke disposable org',1,${now},${now});
INSERT INTO users(id,username,email,organization_id,password_hash,is_admin,is_active,class,role,parent_user_id,created_at,updated_at)
VALUES(${quote(userId)},${quote(staffUsername)},${quote(staffUsername+"@example.invalid")},${quote(orgId)},${quote(staffHash)},0,1,'staff','STAFF',NULL,${now},${now});`);

    const ready = await fetch(`${base}/ready`);
    if (ready.status !== 200) fail(`Local Worker readiness failed with HTTP ${ready.status}.`);

    // Cross-check the exact row the Worker login query should resolve. Emit
    // only boolean diagnostics; never print credential or hash material.
    const lookup = localSql(`SELECT u.id,u.username,u.organization_id,u.password_hash,u.is_admin,u.is_active,u.role
      FROM users u JOIN organizations o ON o.id=u.organization_id AND o.is_active=1
      WHERE (LOWER(u.username)=LOWER('allfather') OR LOWER(u.email)=LOWER('allfather'))
        AND u.is_active=1 LIMIT 1;`);
    const row = lookup[0];
    const diagnostics = {
      found: Boolean(row),
      canonicalId: row?.id === "__allfather__",
      canonicalUsername: typeof row?.username === "string" && row.username.trim().toLowerCase() === "allfather",
      canonicalOrganization: row?.organization_id === "__allfather__",
      canonicalRole: row?.role === "ALL_FATHER",
      active: Number(row?.is_active) === 1,
      admin: Number(row?.is_admin) === 1,
      argon2idHashPresent: typeof row?.password_hash === "string" && row.password_hash.startsWith("$argon2id$"),
      passwordHashVerifies: typeof row?.password_hash === "string" &&
        await argon2Verify({ password, hash: row.password_hash }).catch(() => false),
    };
    console.log(`All-Father login preflight: ${JSON.stringify(diagnostics)}`);

    const requestJson = async (route, body, label) => {
      const response = await fetch(`${base}${route}`, {
        method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body),
      });
      const value = await response.json().catch(() => null);
      return { response, value, label };
    };

    const login = await requestJson("/api/auth/login", {
      username, password, client_type: "admin",
    }, "All-Father login");
    if (login.response.status !== 200) fail(`Valid All-Father credentials returned HTTP ${login.response.status}.`);
    const session = login.value;
    if (typeof session?.access_token !== "string" || typeof session?.refresh_token !== "string" ||
        session?.user?.id !== "__allfather__" || session?.user?.username !== "allfather" ||
        session?.user?.role !== "ALL_FATHER" || session?.user?.is_admin !== true ||
        session?.user?.organization_id !== "__allfather__") {
      fail("Login did not return the canonical All-Father principal and authority.");
    }
    const claims = decodeClaims(session.access_token);
    if (!claims || claims.sub !== "__allfather__" || claims.username !== "allfather" ||
        claims.role !== "ALL_FATHER" || claims.is_admin !== true ||
        claims.organization_id !== "__allfather__" || claims.typ !== "access" ||
        !Number.isSafeInteger(claims.exp) || claims.exp <= Math.floor(Date.now() / 1000)) {
      fail("Issued access token has invalid principal, authority, type, or expiry claims.");
    }
    const protectedResponse = await fetch(`${base}/api/allfather/admins?organization_id=__allfather__`, {
      headers: { authorization: `Bearer ${session.access_token}` },
    });
    if (protectedResponse.status !== 200) fail(`Canonical All-Father protected operation returned HTTP ${protectedResponse.status}.`);
    consumedRefreshHash = createHash("sha256").update(session.refresh_token).digest("hex");
    const refreshResponse = await fetch(`${base}/api/auth/refresh`, {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ refresh_token: session.refresh_token }),
    });
    const refreshed = await refreshResponse.json().catch(() => null);
    if (refreshResponse.status !== 200 || typeof refreshed?.access_token !== "string" ||
        typeof refreshed?.refresh_token !== "string" || refreshed?.user?.id !== "__allfather__" ||
        refreshed?.user?.role !== "ALL_FATHER" || refreshed?.user?.is_admin !== true) {
      fail(`Canonical All-Father refresh returned HTTP ${refreshResponse.status} or invalid authority.`);
    }
    const refreshedProtected = await fetch(`${base}/api/allfather/admins?organization_id=__allfather__`, {
      headers: { authorization: `Bearer ${refreshed.access_token}` },
    });
    if (refreshedProtected.status !== 200) fail(`Refreshed All-Father session failed protected access (HTTP ${refreshedProtected.status}).`);


    const invalid = await requestJson("/api/auth/login", { username, password: password + "x", client_type: "admin" }, "invalid password");
    if (invalid.response.status !== 401 || invalid.value?.access_token !== undefined) fail("Invalid All-Father password was not safely rejected.");
    for (const body of [{ username, client_type: "admin" }, { password, client_type: "admin" }]) {
      const missing = await requestJson("/api/auth/login", body, "missing credential");
      if (missing.response.status !== 401 || missing.value?.access_token !== undefined) fail("Missing credentials were not safely rejected.");
    }

    const legacy = await requestJson("/api/auth/supabase", { access_token: "invalid" }, "retired Supabase route");
    if (legacy.response.ok || legacy.value?.access_token !== undefined || legacy.value?.refresh_token !== undefined) {
      fail("Retired Supabase exchange unexpectedly authenticated a caller.");
    }

    const staffLogin = await requestJson("/api/auth/login", {
      username: staffUsername, password: staffPassword, client_type: "admin",
      id: "__allfather__", organization_id: "__allfather__", role: "ALL_FATHER", is_admin: true, admin: true,
    }, "ordinary account login");
    if (staffLogin.response.status !== 200 || staffLogin.value?.user?.id !== userId ||
        staffLogin.value?.user?.role !== "STAFF" || staffLogin.value?.user?.is_admin !== false ||
        staffLogin.value?.user?.organization_id !== orgId) {
      fail("Client-supplied identity or role metadata elevated the ordinary account.");
    }
    const staffProtected = await fetch(`${base}/api/allfather/admins?organization_id=__allfather__`, {
      headers: { authorization: `Bearer ${staffLogin.value.access_token}` },
    });
    if (staffProtected.status !== 403) fail(`Ordinary account reached All-Father operation (HTTP ${staffProtected.status}).`);

    const forged = await fetch(`${base}/api/users/hierarchy`, { headers: { authorization: "Bearer not-a-valid-token" } });
    if (forged.status !== 401) fail(`Invalid session was not rejected (HTTP ${forged.status}).`);

    console.log("PASS: canonical All-Father login, token claims/expiry, protected operation, invalid credentials, retired exchange, and anti-escalation checks.");
  } finally {
    if (fixtureMayExist) {
      const cleanup = [];
      if (consumedRefreshHash) cleanup.push(`DELETE FROM token_revocations WHERE token_hash=${quote(consumedRefreshHash)};`);
      cleanup.push(`DELETE FROM users WHERE id=${quote(userId)};`);
      cleanup.push(`DELETE FROM organizations WHERE id=${quote(orgId)};`);
      localSql(cleanup.join("\n"));
    }
  }
}

main().catch((error) => {
  if (process.exitCode !== 1) console.error("::error::All-Father smoke test failed; credentials and tokens suppressed.");
  process.exitCode = 1;
});
