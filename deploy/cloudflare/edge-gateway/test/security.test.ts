import { describe, expect, it } from "vitest";
import { argon2id } from "hash-wasm";
import worker from "../src/index";
import { COMMAND_POLICIES, ONYX_ROLES } from "../src/authorization";

const SECRET = "test-onyx-secret";
const ORG = "org-1";
type WorkerEnv = Parameters<typeof worker.fetch>[1];

interface UserRow {
  id: string;
  username: string;
  email: string;
  organization_id: string;
  password_hash?: string;
  is_admin: number;
  is_active: number;
  class: string | null;
  parent_user_id?: string | null;
  role: string;
  supabase_user_id?: string | null;
}
interface OrgRow { id: string; is_active: number }
interface AggregateRow {
  id: string;
  aggregate_type: string;
  version: number;
  lifecycle_epoch: number;
  authority_epoch: number;
  state: string;
  updated_at: number;
  organization_id: string;
}

const encoder = new TextEncoder();
function b64u(input: string | Uint8Array): string {
  const bytes = typeof input === "string" ? encoder.encode(input) : input;
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}
async function signToken(claims: Record<string, unknown>): Promise<string> {
  const payload = { iss: "onyx-cloudflare-worker", aud: "onyx-api", ...claims };
  const input = `${b64u(JSON.stringify({ alg: "HS256", typ: "JWT" }))}.${b64u(JSON.stringify(payload))}`;
  const key = await crypto.subtle.importKey("raw", encoder.encode(SECRET), { name: "HMAC", hash: "SHA-256" }, false, ["sign"]);
  const signature = new Uint8Array(await crypto.subtle.sign("HMAC", key, encoder.encode(input)));
  return `${input}.${b64u(signature)}`;
}
function sessionClaims(overrides: Record<string, unknown> = {}) {
  return {
    sub: "staff-1", username: "staff", organization_id: ORG,
    is_admin: false, class: "STAFF", role: "STAFF", client_type: "desktop",
    exp: Math.floor(Date.now() / 1000) + 3600, jti: crypto.randomUUID(), sid: "session-1", typ: "access",
    ...overrides,
  };
}

class SecurityDb {
  users = new Map<string, UserRow>();
  emptyUserQueries = false;
  organizations = new Map<string, OrgRow>();
  aggregates = new Map<string, AggregateRow>();
  subscriptions = new Map<string, { user_id: string; organization_id: string; endpoint: string }>();
  mobileSettings = new Map<string, { organization_id: string; allowed_classes: string; version: number; updated_by: string; updated_at: number; last_operation_id: string }>();
  fileAssets = new Map<string, { content_hash: string; organization_id: string }>();
  fileAssetLookups = 0;
  idempotency = new Map<string, string>();
  revokedTokens = new Set<string>();
  domainEvents: Array<Record<string, unknown>> = [];
  auditRows: Array<Record<string, unknown>> = [];
  writes: Array<{ sql: string; params: unknown[] }> = [];

  prepare(sql: string) {
    let params: unknown[] = [];
    const db = this;
    const statement: any = {
      sql,
      get boundParams() { return params; },
      bind(...values: unknown[]) { params = values; return statement; },
      first: async () => db.first(sql, params),
      all: async () => ({ results: db.all(sql, params) }),
      run: async () => db.run(sql, params),
    };
    return statement;
  }

  async batch(statements: any[]) {
    const results: Array<{ success: boolean; meta: { changes: number } }> = [];
    let previousChanges = 0;
    for (const statement of statements) {
      const { sql, boundParams: params } = statement;
      const result = this.runBatchStatement(sql, params, previousChanges);
      results.push({ success: true, meta: { changes: result } });
      previousChanges = result;
    }
    return results;
  }

  private first(sql: string, params: unknown[]): any {
    if (sql.includes("FROM mobile_access_settings")) {
      const row = this.mobileSettings.get(String(params[0]));
      return row ? { ...row } : null;
    }
    if (sql.includes("FROM file_assets WHERE content_hash=? AND organization_id=?")) {
      this.fileAssetLookups += 1;
      const row = this.fileAssets.get(String(params[0]));
      return row?.organization_id === params[1] ? { ...row } : null;
    }
    if (sql.includes("FROM aggregates WHERE id=? AND organization_id=? AND aggregate_type=?")) {
      const row = this.aggregates.get(String(params[0]));
      return row?.organization_id === params[1] && row.aggregate_type === params[2] ? { id: row.id } : null;
    }
    if (sql.includes("FROM token_revocations")) {
      return this.revokedTokens.has(String(params[0])) ? { 1: 1 } : null;
    }
    if (sql.includes("FROM idempotency")) {
      const result = this.idempotency.get(String(params[0]));
      return result === undefined ? null : { result };
    }
    if (sql.includes("FROM users u JOIN organizations")) {
      let user: UserRow | undefined;
      if (/WHERE u\.id=\?/i.test(sql)) user = this.users.get(String(params[0]));
      else if (/LOWER\(u\.username\)/i.test(sql)) {
        const identifier = String(params[0]).toLowerCase();
        user = [...this.users.values()].find((row) => row.username.toLowerCase() === identifier || row.email.toLowerCase() === identifier);
      }
      if (!user || user.is_active !== 1) return null;
      const org = this.organizations.get(user.organization_id);
      return org?.is_active === 1 ? { ...user } : null;
    }
    if (sql.includes("FROM users WHERE id=? AND organization_id=?")) {
      const user = this.users.get(String(params[0]));
      if (!user || user.organization_id !== params[1] || user.is_active !== 1) return null;
      const select = sql.slice(sql.indexOf("SELECT ") + 7, sql.indexOf(" FROM ")).split(",").map((value) => value.trim().replace(/^u\\./, ""));
      return Object.fromEntries(select.map((column) => [column, (user as unknown as Record<string, unknown>)[column] ?? null]));
    }
    if (sql.includes("FROM aggregates WHERE id=? AND organization_id=?")) {
      const row = this.aggregates.get(String(params[0]));
      return row?.organization_id === params[1] ? { ...row } : null;
    }
    return null;
  }

  private all(sql: string, params: unknown[]): unknown[] {
    if (sql.includes("FROM aggregates")) {
      let rows = [...this.aggregates.values()];
      if (sql.includes("WHERE organization_id=?")) rows = rows.filter((row) => row.organization_id === String(params[0]));
      if (sql.includes("aggregate_type=?")) {
        const before = sql.slice(0, sql.indexOf("aggregate_type=?"));
        const p = (before.match(/\?/g) ?? []).length;
        rows = rows.filter((row) => row.aggregate_type === String(params[p]));
      }
      if (/AND id=\?/i.test(sql) && sql.includes("aggregate_type=?")) {
        const before = sql.slice(0, sql.indexOf("id=?"));
        const p = (before.match(/\?/g) ?? []).length;
        const value = params[p];
        if (typeof value === "string") rows = rows.filter((row) => row.id === value);
      }
      if (sql.includes("json_valid(state)")) {
        const predicate = sql.indexOf("json_extract(state,'$.owner_id')");
        const firstOwnerParam = (sql.slice(0, predicate).match(/\?/g) ?? []).length;
        const ownerId = String(params[firstOwnerParam]);
        rows = rows.filter((row) => {
          try {
            const state = JSON.parse(row.state) as Record<string, unknown>;
            const owners = ["owner_id","owner_user_id","assignee_id","assigned_to","user_id","recipient_user_id"]
              .map((key) => state[key]).filter((value) => typeof value === "string" && value.length > 0) as string[];
            const nested = state.owner && typeof state.owner === "object" ? (state.owner as Record<string, unknown>).user_id : undefined;
            if (typeof nested === "string" && nested) owners.push(nested);
            return owners.length > 0 && owners.every((id) => id === ownerId);
          } catch { return false; }
        });
      }
      return rows;
    }
    if (sql.includes("FROM users")) {
      if (this.emptyUserQueries) return [];
      let rows = [...this.users.values()];
      if (sql.includes("organization_id=?")) rows = rows.filter((row) => row.organization_id === String(params[0]));
      if (sql.includes("is_active=1")) rows = rows.filter((row) => row.is_active === 1);
      if (sql.includes("id IN (?,?)")) {
        const ids = new Set(params.slice(1).map(String));
        rows = rows.filter((row) => ids.has(row.id));
      }
      if (sql.includes("ORDER BY id")) rows.sort((a, b) => a.id.localeCompare(b.id));
      if (sql.includes("ORDER BY username")) rows.sort((a, b) => a.username.localeCompare(b.username));
      const select = sql.slice(sql.indexOf("SELECT ") + 7, sql.indexOf(" FROM ")).split(",").map((value) => value.trim().replace(/^u\\./, ""));
      return rows.map((row) => Object.fromEntries(select.map((column) => [column, (row as unknown as Record<string, unknown>)[column] ?? null])));
    }
    return [];
  }

  private run(sql: string, params: unknown[]) {
    if (sql.includes("INSERT INTO push_subscriptions")) {
      const id = String(params[0]);
      const user_id = String(params[1]);
      const organization_id = String(params[2]);
      const existing = this.subscriptions.get(id);
      if (existing && (existing.user_id !== user_id || existing.organization_id !== organization_id)) {
        return { success: true, meta: { changes: 0 } };
      }
      this.subscriptions.set(id, { user_id, organization_id, endpoint: String(params[3]) });
      this.writes.push({ sql, params });
      return { success: true, meta: { changes: 1 } };
    }
    this.writes.push({ sql, params });
    if (sql.includes("INSERT OR IGNORE INTO token_revocations")) {
      const hash = String(params[0]);
      const changes = this.revokedTokens.has(hash) ? 0 : 1;
      this.revokedTokens.add(hash);
      return { success: true, meta: { changes } };
    }
    return { success: true, meta: { changes: 1 } };
  }

  private runBatchStatement(sql: string, params: unknown[], previousChanges: number): number {
    if (sql.startsWith("INSERT INTO mobile_access_settings")) {
      const org = String(params[0]);
      const existing = this.mobileSettings.get(org);
      const expected = Number(params[5]);
      if ((!existing && expected !== 0) || (existing && existing.version !== expected)) return 0;
      this.mobileSettings.set(org, {
        organization_id: org, allowed_classes: String(params[1]),
        version: existing ? existing.version + 1 : 1, updated_by: String(params[2]),
        updated_at: Number(params[3]), last_operation_id: String(params[4]),
      });
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("UPDATE users SET class=?")) {
      const user = this.users.get(String(params[2]));
      if (!user || user.organization_id !== params[3] || user.is_active !== 1) return 0;
      user.class = params[0] === null ? null : String(params[0]);
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("INSERT INTO audit_log") && sql.includes("mobile_access_settings")) {
      const row = this.mobileSettings.get(String(params[6]));
      if (!row || row.last_operation_id !== String(params[7]) || row.version !== Number(params[8])) return 0;
      this.auditRows.push({ organization_id: params[0], user_id: params[1], action: params[2] });
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("INSERT INTO audit_log") && sql.includes("WHERE changes()=1")) {
      if (previousChanges !== 1) return 0;
      this.auditRows.push({ organization_id: params[0], user_id: params[1], action: params[2] });
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("INSERT OR IGNORE INTO aggregates")) {
      const id = String(params[0]);
      if (this.aggregates.has(id)) return 0;
      this.aggregates.set(id, {
        id, aggregate_type: String(params[1]), version: Number(params[2]),
        lifecycle_epoch: Number(params[3]), authority_epoch: Number(params[4]),
        state: String(params[5]), updated_at: Number(params[6]), organization_id: String(params[7]),
      });
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("UPDATE aggregates SET")) {
      const row = this.aggregates.get(String(params[6]));
      if (!row || row.organization_id !== params[7] || row.aggregate_type !== params[8] ||
          row.version !== Number(params[9]) || row.lifecycle_epoch !== Number(params[10]) ||
          row.authority_epoch !== Number(params[11])) return 0;
      this.aggregates.set(row.id, {
        ...row, aggregate_type: String(params[0]), version: Number(params[1]),
        lifecycle_epoch: Number(params[2]), authority_epoch: Number(params[3]),
        state: String(params[4]), updated_at: Number(params[5]),
      });
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("INSERT INTO domain_events") && previousChanges === 1) {
      this.domainEvents.push({
        event_id: params[0], aggregate_id: params[1], event_type: params[3],
        organization_id: params[10],
      });
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("INSERT INTO idempotency")) {
      const eventId = String(params[3]);
      if (!this.domainEvents.some((event) => event.event_id === eventId && event.organization_id === params[4])) return 0;
      if (this.idempotency.has(String(params[0]))) throw new Error("UNIQUE constraint failed: idempotency.operation_id");
      this.idempotency.set(String(params[0]), String(params[1]));
      this.writes.push({ sql, params });
      return 1;
    }
    if (sql.startsWith("INSERT INTO audit_log")) {
      const eventId = String(params[6]);
      if (!this.domainEvents.some((event) => event.event_id === eventId && event.organization_id === params[7])) return 0;
      this.auditRows.push({ organization_id: params[0], user_id: params[1], action: params[2] });
      this.writes.push({ sql, params });
      return 1;
    }
    return 0;
  }
}

function makeDb(role = "STAFF") {
  const db = new SecurityDb();
  db.organizations.set(ORG, { id: ORG, is_active: 1 });
  db.users.set("staff-1", {
    id: "staff-1", username: "staff", email: "staff@example.test", organization_id: ORG,
    is_admin: role === "ORGANIZATION_ADMIN" ? 1 : 0, is_active: 1,
    class: role, role, supabase_user_id: null,
  });
  db.aggregates.set("task-1", {
    id: "task-1", aggregate_type: "task", version: 1, lifecycle_epoch: 0,
    authority_epoch: 0, state: JSON.stringify({ status: "Ready", owner_id: "staff-1", title: "My task" }),
    updated_at: Date.now(), organization_id: ORG,
  });
  return db;
}
function makeEnv(db: SecurityDb): WorkerEnv {
  return { DB: db, ONYX_JWT_SECRET: SECRET, ONYX_CORS_ORIGINS: "https://app.example" } as unknown as WorkerEnv;
}
async function postCommand(clientType: string, db: SecurityDb, extra: Record<string, unknown> = {}) {
  const currentUser = db.users.get("staff-1");
  const token = await signToken(sessionClaims({
    client_type: clientType,
    role: currentUser?.role ?? "STAFF",
    is_admin: Boolean(currentUser?.is_admin),
  }));
  const envelope = {
    command_id: "cmd-1", operation_id: "op-1", command_type: "StartTask",
    schema_version: "1.0", target: { id: "task-1", type: "task", organization_id: ORG },
    expected_version: 1, expected_lifecycle_epoch: 0, expected_authority_epoch: 0,
    payload: { reason: "work started" }, ...extra,
  };
  return worker.fetch(new Request("https://worker.test/api/command", {
    method: "POST", headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
    body: JSON.stringify(envelope),
  }), makeEnv(db));
}

describe("Worker authorization and session security", () => {
  it("does not let client_type change a Staff command decision", async () => {
    const desktopDb = makeDb();
    const observerDb = makeDb();
    const desktop = await postCommand("mobile", desktopDb);
    const observer = await postCommand("mobile_observer", observerDb);

    expect(desktop.status).toBe(200);
    expect(observer.status).toBe(200);
    expect(desktopDb.aggregates.get("task-1")?.version).toBe(2);
    expect(observerDb.aggregates.get("task-1")?.version).toBe(2);
    expect(JSON.parse(desktopDb.aggregates.get("task-1")!.state).status).toBe("Active");
    expect(JSON.parse(observerDb.aggregates.get("task-1")!.state).status).toBe("Active");
    expect(desktopDb.domainEvents).toHaveLength(1);
    expect(observerDb.domainEvents).toHaveLength(1);
  });

  it("applies Staff ownership inside protected query reads", async () => {
    const db = makeDb();
    db.aggregates.set("task-other", {
      id: "task-other", aggregate_type: "task", version: 1, lifecycle_epoch: 0,
      authority_epoch: 0, state: JSON.stringify({ status: "Ready", owner_id: "staff-2", title: "Other staff task" }),
      updated_at: Date.now(), organization_id: ORG,
    });
    db.aggregates.set("task-foreign", {
      id: "task-foreign", aggregate_type: "task", version: 1, lifecycle_epoch: 0,
      authority_epoch: 0, state: JSON.stringify({ status: "Ready", owner_id: "staff-1", title: "Foreign tenant task" }),
      updated_at: Date.now(), organization_id: "org-foreign",
    });
    const token = await signToken(sessionClaims());
    const envelope = b64u(JSON.stringify({ query_id: "q-1", query_type: "task.list", organization_id: ORG }));
    const response = await worker.fetch(new Request("https://worker.test/api/query?envelope=" + encodeURIComponent(envelope), {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(200);
    const body = await response.json() as { data: Array<{ id: string }> };
    expect(body.data.map((item) => item.id)).toEqual(["task-1"]);
  });

  it("denies cross-tenant resource scope for every role × registered command", async () => {
    for (const role of ONYX_ROLES) {
      const db = makeDb(role === "ALL_FATHER" ? "STAFF" : role);
      let token: string;
      if (role === "ALL_FATHER") {
        db.users.clear();
        db.organizations.set("org-root", { id: "org-root", is_active: 1 });
        db.users.set("__allfather__", {
          id: "__allfather__", username: "allfather", email: "owner@example.test",
          organization_id: "org-root", is_admin: 1, is_active: 1, class: "ALL_FATHER",
          role: "ALL_FATHER", supabase_user_id: "subject-1",
        });
        token = await signToken(sessionClaims({
          sub: "__allfather__", username: "allfather", organization_id: "org-root",
          is_admin: true, role: "ALL_FATHER", identity_sub: "subject-1",
        }));
      } else {
        token = await signToken(sessionClaims({
          role,
          is_admin: role === "ORGANIZATION_ADMIN",
        }));
      }

      for (const [action, policy] of Object.entries(COMMAND_POLICIES)) {
        const operationId = `scope-${role}-${action}`;
        const response = await worker.fetch(new Request("https://worker.test/api/command", {
          method: "POST",
          headers: { authorization: "Bearer " + token, "content-type": "application/json" },
          body: JSON.stringify({
            command_id: "cmd-scope",
            operation_id: operationId,
            command_type: action,
            target: { id: "foreign-resource", type: policy.targetType, organization_id: "foreign-org" },
            expected_version: 0,
            expected_lifecycle_epoch: 0,
            expected_authority_epoch: 0,
            payload: {},
          }),
        }), db ? makeEnv(db) : makeEnv(makeDb()));

        expect(response.status, `${role} / ${action} / cross-tenant`).toBe(403);
        expect(db.writes, `${role} / ${action} writes`).toHaveLength(0);
        expect(db.domainEvents, `${role} / ${action} events`).toHaveLength(0);
      }
    }
  });

  it("denies Staff updates to same-tenant resources owned by another Staff member", async () => {
    const db = makeDb();
    db.aggregates.set("task-other", {
      id: "task-other", aggregate_type: "task", version: 1, lifecycle_epoch: 0,
      authority_epoch: 0, state: JSON.stringify({ status: "Ready", owner_id: "staff-2", title: "Other staff task" }),
      updated_at: Date.now(), organization_id: ORG,
    });
    const response = await postCommand("mobile", db, {
      target: { id: "task-other", type: "task", organization_id: ORG },
    });
    expect(response.status).toBe(403);
    expect(db.aggregates.get("task-other")?.version).toBe(1);
    expect(db.domainEvents).toHaveLength(0);
    expect(db.auditRows).toHaveLength(0);
    expect(db.writes).toHaveLength(0);
  });

  it("denies unknown and delete-shaped commands without mutation", async () => {
    const db = makeDb();
    const response = await postCommand("desktop", db, { command_type: "DeleteTask" });
    const body = await response.json() as Record<string, unknown>;

    expect(response.status).toBe(403);
    expect(body.error).toBe("UNKNOWN_COMMAND");
    expect(db.aggregates.get("task-1")?.version).toBe(1);
    expect(db.domainEvents).toHaveLength(0);
    expect(db.auditRows).toHaveLength(0);
    expect(db.writes).toHaveLength(0);
  });

  it("denies a Staff All-Father-only route without mutation", async () => {
    const db = makeDb();
    const token = await signToken(sessionClaims());
    const response = await worker.fetch(new Request("https://worker.test/api/allfather/organizations", {
      method: "POST",
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
      body: JSON.stringify({ name: "Attacker controlled" }),
    }), makeEnv(db));

    expect(response.status).toBe(403);
    expect(db.writes).toHaveLength(0);
  });

  it("denies a cross-tenant target before reading or mutating the resource", async () => {
    const db = makeDb();
    const response = await postCommand("desktop", db, {
      target: { id: "task-1", type: "task", organization_id: "other-org" },
    });
    expect(response.status).toBe(403);
    expect(db.aggregates.get("task-1")?.version).toBe(1);
    expect(db.writes).toHaveLength(0);
  });

  it("rejects an inactive account or inactive organization on each protected request", async () => {
    const inactiveAccountDb = makeDb();
    inactiveAccountDb.users.get("staff-1")!.is_active = 0;
    const inactiveAccount = await postCommand("desktop", inactiveAccountDb);
    expect(inactiveAccount.status).toBe(401);
    expect(inactiveAccountDb.writes).toHaveLength(0);

    const inactiveOrgDb = makeDb();
    inactiveOrgDb.organizations.get(ORG)!.is_active = 0;
    const inactiveOrg = await postCommand("desktop", inactiveOrgDb);
    expect(inactiveOrg.status).toBe(401);
    expect(inactiveOrgDb.writes).toHaveLength(0);
  });

  it("rejects All-Father access tokens whose subject no longer matches the D1 mapping", async () => {
    const db = new SecurityDb();
    db.organizations.set("__allfather__", { id: "__allfather__", is_active: 1 });
    db.users.set("__allfather__", {
      id: "__allfather__", username: "allfather", email: "owner@example.test",
      organization_id: "__allfather__", is_admin: 1, is_active: 1, class: "ALL_FATHER",
      role: "ALL_FATHER", supabase_user_id: "subject-current",
    });
    const token = await signToken(sessionClaims({
      sub: "__allfather__", username: "allfather", organization_id: "__allfather__",
      is_admin: true, role: "ALL_FATHER", identity_sub: "subject-old",
    }));
    const response = await worker.fetch(new Request("https://worker.test/api/query?envelope=" +
      encodeURIComponent(b64u(JSON.stringify({ query_id: "q-1", query_type: "dashboard.summary", organization_id: "__allfather__" }))), {
      headers: { authorization: `Bearer ${token}` },
    }), makeEnv(db));

    expect(response.status).toBe(401);
    expect(db.writes).toHaveLength(0);
  });

  it("rejects malformed, expired, wrong-issuer, and wrong-audience ONYX tokens", async () => {
    const malformed = "not-a-jwt";
    const wrongIssuer = await signToken(sessionClaims({ iss: "https://attacker.example" }));
    const wrongAudience = await signToken(sessionClaims({ aud: "another-service" }));
    const expired = await signToken(sessionClaims({ exp: Math.floor(Date.now() / 1000) - 1 }));
    for (const token of [malformed, wrongIssuer, wrongAudience, expired]) {
      const db = makeDb();
      const response = await worker.fetch(new Request("https://worker.test/api/query", {
        headers: { authorization: "Bearer " + token },
      }), makeEnv(db));
      expect(response.status).toBe(401);
      expect(db.writes).toHaveLength(0);
    }
  });

  it("rotates a refresh token once and rejects replay", async () => {
    const db = makeDb();
    const token = await signToken(sessionClaims({ typ: "refresh" }));
    const request = () => new Request("https://worker.test/api/auth/refresh", {
      method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ refresh_token: token }),
    });

    const first = await worker.fetch(request(), makeEnv(db));
    expect(first.status).toBe(200);
    const second = await worker.fetch(request(), makeEnv(db));
    expect(second.status).toBe(401);
  });

  it("prevents one user from reassigning another user's push-subscription ID", async () => {
    const db = makeDb();
    const ownerToken = await signToken(sessionClaims());
    const create = await worker.fetch(new Request("https://worker.test/api/push/subscriptions", {
      method: "POST",
      headers: { authorization: "Bearer " + ownerToken, "content-type": "application/json" },
      body: JSON.stringify({ id: "push-1", endpoint: "https://push.example/one", p256dh: "key-1", auth: "auth-1" }),
    }), makeEnv(db));
    expect(create.status).toBe(200);

    db.users.set("staff-2", {
      id: "staff-2", username: "staff2", email: "staff2@example.test", organization_id: ORG,
      is_admin: 0, is_active: 1, class: "STAFF", role: "STAFF", supabase_user_id: null,
    });
    const otherToken = await signToken(sessionClaims({ sub: "staff-2", username: "staff2", jti: crypto.randomUUID() }));
    const overwrite = await worker.fetch(new Request("https://worker.test/api/push/subscriptions", {
      method: "POST",
      headers: { authorization: "Bearer " + otherToken, "content-type": "application/json" },
      body: JSON.stringify({ id: "push-1", endpoint: "https://push.example/attacker", p256dh: "key-x", auth: "auth-x" }),
    }), makeEnv(db));

    expect(overwrite.status).toBe(409);
    expect(db.subscriptions.get("push-1")).toEqual({
      user_id: "staff-1", organization_id: ORG, endpoint: "https://push.example/one",
    });
  });

  it("revokes signed access and refresh tokens on logout but ignores arbitrary strings", async () => {
    const db = makeDb();
    const access = await signToken(sessionClaims());
    const refresh = await signToken(sessionClaims({ typ: "refresh" }));
    const response = await worker.fetch(new Request("https://worker.test/api/auth/logout", {
      method: "POST",
      headers: { authorization: "Bearer " + access, "content-type": "application/json" },
      body: JSON.stringify({}),
    }), makeEnv(db));
    expect(response.status).toBe(200);
    expect(db.revokedTokens.size).toBe(2);
    const attempt = await worker.fetch(new Request("https://worker.test/api/query", {
      headers: { authorization: "Bearer " + access },
    }), makeEnv(db));
    expect(attempt.status).toBe(401);
    const refreshReplay = await worker.fetch(new Request("https://worker.test/api/auth/refresh", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ refresh_token: refresh }),
    }), makeEnv(db));
    expect(refreshReplay.status).toBe(401);

    const junkDb = makeDb();
    const junk = await worker.fetch(new Request("https://worker.test/api/auth/logout", {
      method: "POST",
      headers: { authorization: "Bearer definitely-not-a-token", "content-type": "application/json" },
      body: JSON.stringify({ refresh_token: "also-not-a-token" }),
    }), makeEnv(junkDb));
    expect(junk.status).toBe(200);
    expect(junkDb.writes).toHaveLength(0);
  });



  it("serves and durably replaces mobile-access configuration with role, validation, audit and version checks", async () => {
    const adminDb = makeDb("ORGANIZATION_ADMIN");
    const adminToken = await signToken(sessionClaims({
      role: "ORGANIZATION_ADMIN", is_admin: true,
    }));
    const request = (method: string, body?: unknown, token = adminToken) => new Request("https://worker.test/api/admin/mobile-access", {
      method, headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      ...(body === undefined ? {} : { body: JSON.stringify(body) }),
    });

    const initial = await worker.fetch(request("GET"), makeEnv(adminDb));
    expect(initial.status).toBe(200);
    expect(await initial.json()).toEqual({
      allowed_classes: [], version: 0, updated_at: null,
      enforcement_status: "disabled_untrusted_client_type",
    });

    const invalid = await worker.fetch(request("PUT", { allowed_classes: ["unknown"], expected_version: 0 }), makeEnv(adminDb));
    expect(invalid.status).toBe(400);
    expect(adminDb.writes).toHaveLength(0);

    const saved = await worker.fetch(request("PUT", { allowed_classes: ["staff", "team_leader"], expected_version: 0 }), makeEnv(adminDb));
    expect(saved.status).toBe(200);
    expect(await saved.json()).toMatchObject({ allowed_classes: ["team_leader", "staff"], version: 1 });
    expect(adminDb.mobileSettings.get(ORG)?.allowed_classes).toBe(JSON.stringify(["team_leader", "staff"]));
    expect(adminDb.auditRows).toHaveLength(1);

    const stale = await worker.fetch(request("PUT", { allowed_classes: [], expected_version: 0 }), makeEnv(adminDb));
    expect(stale.status).toBe(409);
    expect(adminDb.mobileSettings.get(ORG)?.version).toBe(1);

    const staffDb = makeDb();
    const staffToken = await signToken(sessionClaims());
    const denied = await worker.fetch(request("GET", undefined, staffToken), makeEnv(staffDb));
    expect(denied.status).toBe(403);
    const deniedWrite = await worker.fetch(request("PUT", {
      allowed_classes: ["staff"], expected_version: 0,
    }, staffToken), makeEnv(staffDb));
    expect(deniedWrite.status).toBe(403);
    expect(staffDb.mobileSettings.size).toBe(0);
    expect(staffDb.writes).toHaveLength(0);
    expect(staffDb.auditRows).toHaveLength(0);
  });

  it("returns hierarchy as the minimal sorted JSON array expected by Rust and Android", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/users/hierarchy", {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
    expect(body).toEqual([{ id: "staff-1", parent_user_id: null, is_admin: true }]);
  });

  it("returns an honest 503 for mobile-access storage failure", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const originalPrepare = db.prepare.bind(db);
    Object.assign(db, {
      prepare: (sql: string) => {
        if (sql.includes("mobile_access_settings")) throw new Error("simulated D1 outage");
        return originalPrepare(sql);
      },
    });
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/mobile-access", {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(503);
    expect((await response.json() as Record<string, unknown>).error).toBe("MOBILE_ACCESS_STORAGE_UNAVAILABLE");
    expect(db.writes.some((write) => write.sql.includes("mobile_access_settings"))).toBe(false);
  });

  it("returns the canonical empty hierarchy array when no visible rows are returned", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    db.emptyUserQueries = true;
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/users/hierarchy", {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual([]);
  });

  it("denies Staff policy and legal-hold creation without persistence or audit", async () => {
    const db = makeDb();
    const token = await signToken(sessionClaims());
    const policy = await worker.fetch(new Request("https://worker.test/api/admin/policies", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ name: "Unauthorized policy" }),
    }), makeEnv(db));
    const hold = await worker.fetch(new Request("https://worker.test/api/admin/legal-holds", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({
        target_id: "123e4567-e89b-42d3-a456-426614174000",
        target_type: "task",
        reason: "Unauthorized hold",
      }),
    }), makeEnv(db));
    expect(policy.status).toBe(403);
    expect(hold.status).toBe(403);
    expect(db.writes).toHaveLength(0);
    expect(db.domainEvents).toHaveLength(0);
    expect(db.auditRows).toHaveLength(0);
  });

  it("makes policy creation idempotent and rejects key reuse with a different payload", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const create = (name: string) => worker.fetch(new Request("https://worker.test/api/admin/policies", {
      method: "POST", headers: {
        authorization: "Bearer " + token, "content-type": "application/json", "idempotency-key": "same-create-key",
      },
      body: JSON.stringify({ name }),
    }), makeEnv(db));
    const first = await create("Operations Policy");
    const firstBody = await first.json() as { policy_id: string };
    const repeated = await create("Operations Policy");
    const repeatedBody = await repeated.json() as { policy_id: string };
    expect(first.status).toBe(201);
    expect(repeated.status).toBe(201);
    expect(repeatedBody.policy_id).toBe(firstBody.policy_id);
    expect([...db.aggregates.values()].filter((row) => row.aggregate_type === "policy")).toHaveLength(1);
    expect(db.domainEvents).toHaveLength(1);

    const conflict = await create("Different Policy");
    expect(conflict.status).toBe(409);
    expect((await conflict.json() as Record<string, unknown>).error).toBe("IDEMPOTENCY_CONFLICT");
    expect([...db.aggregates.values()].filter((row) => row.aggregate_type === "policy")).toHaveLength(1);
  });

  it("includes active direct reports for Staff within the same organization hierarchy", async () => {
    const db = makeDb();
    db.users.set("staff-2", {
      id: "staff-2", username: "report", email: "report@example.test",
      organization_id: ORG, is_admin: 0, is_active: 1, class: "staff",
      role: "STAFF", parent_user_id: "staff-1",
    });
    db.users.set("staff-inactive", {
      id: "staff-inactive", username: "inactive", email: "inactive@example.test",
      organization_id: ORG, is_admin: 0, is_active: 0, class: "staff",
      role: "STAFF", parent_user_id: "staff-1",
    });
    db.users.set("other-org-user", {
      id: "other-org-user", username: "foreign", email: "foreign@example.test",
      organization_id: "other-org", is_admin: 0, is_active: 1, class: "staff",
      role: "STAFF", parent_user_id: null,
    });
    const token = await signToken(sessionClaims());
    const response = await worker.fetch(new Request("https://worker.test/api/users/hierarchy", {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual([
      { id: "staff-2", parent_user_id: "staff-1", is_admin: false },
      { id: "staff-1", parent_user_id: null, is_admin: false },
    ]);
  });

  it("returns an empty hierarchy array for the sentinel-ID All-Father principal", async () => {
    const db = new SecurityDb();
    db.organizations.set("__allfather__", { id: "__allfather__", is_active: 1 });
    db.users.set("__allfather__", {
      id: "__allfather__", username: "allfather", email: "allfather@example.test",
      organization_id: "__allfather__", is_admin: 1, is_active: 1, class: "ALL_FATHER",
      role: "ALL_FATHER", supabase_user_id: "subject-current",
    });
    const token = await signToken(sessionClaims({
      sub: "__allfather__", username: "allfather", organization_id: "__allfather__",
      is_admin: true, role: "ALL_FATHER", identity_sub: "subject-current",
    }));
    const response = await worker.fetch(new Request("https://worker.test/api/users/hierarchy", {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual([]);
  });

  it("rejects invalid policy payloads without persistence or audit", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    for (const payload of [
      { name: " " },
      { name: "Valid name", scope: { Organization: ORG } },
      { name: "x".repeat(201) },
    ]) {
      const response = await worker.fetch(new Request("https://worker.test/api/admin/policies", {
        method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
        body: JSON.stringify(payload),
      }), makeEnv(db));
      expect(response.status).toBe(400);
    }
    expect([...db.aggregates.values()].filter((row) => row.aggregate_type === "policy")).toHaveLength(0);
    expect(db.domainEvents).toHaveLength(0);
    expect(db.auditRows).toHaveLength(0);
    expect(db.writes).toHaveLength(0);
  });

  it("persists policy creation through the domain command path and returns canonical saved state", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/policies", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ name: "  Operations Policy  " }),
    }), makeEnv(db));
    expect(response.status).toBe(201);
    const body = await response.json() as { policy_id: string; policy: Record<string, unknown>; version: number };
    expect(body.policy_id).toBeTruthy();
    expect(body.version).toBe(1);
    expect(body.policy).toMatchObject({ name: "Operations Policy", status: "Active", versions: [] });
    expect(db.aggregates.get(body.policy_id)?.aggregate_type).toBe("policy");
    expect(db.domainEvents).toHaveLength(1);
    expect(db.auditRows).toHaveLength(1);
  });

  it("rejects file-asset legal holds until the UUID/content-hash identity contract is resolved", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/legal-holds", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({
        target_id: "123e4567-e89b-42d3-a456-426614174000",
        target_type: "file_asset",
        reason: "preserve evidence",
      }),
    }), makeEnv(db));
    expect(response.status).toBe(400);
    expect((await response.json() as Record<string, unknown>).error).toBe("INVALID_LEGAL_HOLD");
    expect(db.writes).toHaveLength(0);
    expect(db.domainEvents).toHaveLength(0);
    expect(db.auditRows).toHaveLength(0);
  });

  it("rejects cross-tenant legal-hold targets without creating a hold or audit", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const foreignTarget = "123e4567-e89b-42d3-a456-426614174000";
    db.aggregates.set(foreignTarget, {
      id: foreignTarget, aggregate_type: "task", version: 1, lifecycle_epoch: 0,
      authority_epoch: 0, state: JSON.stringify({ status: "Ready" }), updated_at: Date.now(), organization_id: "other-org",
    });
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/legal-holds", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ target_id: foreignTarget, target_type: "task", reason: "preserve evidence" }),
    }), makeEnv(db));
    expect(response.status).toBe(404);
    expect([...db.aggregates.values()].filter((row) => row.aggregate_type === "legal_hold")).toHaveLength(0);
    expect(db.domainEvents).toHaveLength(0);
    expect(db.auditRows).toHaveLength(0);
    expect(db.writes).toHaveLength(0);
  });

  it("creates a tenant-scoped legal hold with event, aggregate and audit when no authorizing policy is supplied", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const targetId = "123e4567-e89b-42d3-a456-426614174000";
    db.aggregates.set(targetId, {
      id: targetId, aggregate_type: "task", version: 1, lifecycle_epoch: 0,
      authority_epoch: 0, state: JSON.stringify({ status: "Ready", owner_id: "staff-1" }),
      updated_at: Date.now(), organization_id: ORG,
    });
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/legal-holds", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ target_id: targetId, target_type: "task", reason: "Preserve relevant records." }),
    }), makeEnv(db));
    expect(response.status).toBe(201);
    const body = await response.json() as { legal_hold_id: string; legal_hold: Record<string, unknown>; version: number };
    expect(body.legal_hold_id).toBeTruthy();
    expect(body.version).toBe(1);
    expect(body.legal_hold).toMatchObject({
      status: "Applied",
      target: { target_id: targetId, target_type: "task" },
      reason: "Preserve relevant records.",
    });
    expect(db.aggregates.get(body.legal_hold_id)?.aggregate_type).toBe("legal_hold");
    expect(db.domainEvents).toHaveLength(1);
    expect(db.auditRows).toHaveLength(1);
    expect(db.writes.some((write) => write.sql.startsWith("INSERT OR IGNORE INTO aggregates"))).toBe(true);
  });

  it("does not trust client_type for authorization or let it override the persisted mobile policy", async () => {
    const db = makeDb();
    const password = "secure-test-password";
    const salt = crypto.getRandomValues(new Uint8Array(16));
    db.users.get("staff-1")!.class = "staff";
    db.users.get("staff-1")!.password_hash = String(await argon2id({
      password, salt, parallelism: 1, iterations: 3, memorySize: 65536, hashLength: 32, outputType: "encoded",
    }));
    db.mobileSettings.set(ORG, {
      organization_id: ORG, allowed_classes: JSON.stringify([]), version: 1,
      updated_by: "admin-1", updated_at: Date.now(), last_operation_id: "test-operation",
    });
    const login = (client_type: string) => worker.fetch(new Request("https://worker.test/api/auth/login", {
      method: "POST", headers: { "content-type": "application/json" },
      body: JSON.stringify({ username: "staff", password, client_type }),
    }), makeEnv(db));

    const mobile = await login("mobile");
    const desktop = await login("desktop");
    expect(mobile.status).toBe(200);
    expect(desktop.status).toBe(200);
    const mobileBody = await mobile.json() as { access_token: string; user: { role: string } };
    const desktopBody = await desktop.json() as { access_token: string; user: { role: string } };
    expect(mobileBody.user.role).toBe("STAFF");
    expect(desktopBody.user.role).toBe("STAFF");

    const denied = await worker.fetch(new Request("https://worker.test/api/admin/policies", {
      method: "POST", headers: {
        authorization: "Bearer " + mobileBody.access_token,
        "content-type": "application/json",
      },
      body: JSON.stringify({ name: "A Staff caller must not create this" }),
    }), makeEnv(db));
    expect(denied.status).toBe(403);
    expect([...db.aggregates.values()].filter((row) => row.aggregate_type === "policy")).toHaveLength(0);
    expect(db.domainEvents).toHaveLength(0);
    expect(db.auditRows).toHaveLength(0);
  });

  it("successfully assigns an operational class and writes its audit record", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/users/staff-1/class", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ class: "supervisor" }),
    }), makeEnv(db));
    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({ success: true, class: "supervisor" });
    expect(db.users.get("staff-1")?.class).toBe("supervisor");
    expect(db.auditRows).toHaveLength(1);
    expect(db.auditRows[0]).toMatchObject({ action: "user.class.update" });
  });

  it("class assignment is tenant-scoped and audited because mobile access grants are class-based", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    db.users.set("staff-2", {
      id: "staff-2", username: "staff2", email: "staff2@example.test", organization_id: "other-org",
      is_admin: 0, is_active: 1, class: null, role: "STAFF",
    });
    const token = await signToken(sessionClaims({ role: "ORGANIZATION_ADMIN", is_admin: true }));
    const request = () => worker.fetch(new Request("https://worker.test/api/admin/users/staff-2/class", {
      method: "POST", headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ class: "staff" }),
    }), makeEnv(db));
    const response = await request();
    expect(response.status).toBe(404);
    expect(db.users.get("staff-2")?.class).toBeNull();
    expect(db.auditRows).toHaveLength(0);
  });

  it("does not allow Admin or Staff to mutate client-supplied state/security fields", async () => {
    for (const key of ["state", "status", "role", "is_admin", "organization_id"]) {
      const db = makeDb("ORGANIZATION_ADMIN");
      const response = await postCommand("desktop", db, { payload: { [key]: "forged" } });
      expect(response.status, key).toBe(400);
      expect(db.aggregates.get("task-1")?.version).toBe(1);
      expect(db.writes).toHaveLength(0);
    }
  });
});


describe("GET /api/admin/users", () => {
  it("returns minimal user DTOs for the caller's organization only", async () => {
    const db = makeDb("ORGANIZATION_ADMIN");
    db.users.set("staff-2", {
      id: "staff-2", username: "staff-two", email: "staff2@example.test",
      organization_id: ORG, password_hash: "must-not-leak", is_admin: 0,
      is_active: 0, class: "staff", parent_user_id: "staff-1", role: "STAFF",
    });
    db.users.set("foreign-staff", {
      id: "foreign-staff", username: "foreign", email: "foreign@example.test",
      organization_id: "org-2", password_hash: "secret", is_admin: 0,
      is_active: 1, class: "staff", parent_user_id: null, role: "STAFF",
    });
    const token = await signToken(sessionClaims({
      sub: "staff-1", username: "staff", organization_id: ORG,
      role: "ORGANIZATION_ADMIN", is_admin: true,
    }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/users", {
      headers: { authorization: `Bearer ${token}` },
    }), makeEnv(db));
    expect(response.status).toBe(200);
    const users = await response.json() as Array<Record<string, unknown>>;
    expect(users).toHaveLength(2);
    expect(users.map((user) => user.id)).toEqual(["staff-1", "staff-2"]);
    expect(users.every((user) => user.organization_id === ORG)).toBe(true);
    expect(users.every((user) => !("password_hash" in user) && !("email" in user))).toBe(true);
    expect(users.find((user) => user.id === "staff-2")).toMatchObject({
      is_admin: false, is_manager: false, is_active: false,
      class: "staff", parent_user_id: "staff-1",
    });
  });

  it("denies Staff access", async () => {
    const db = makeDb("STAFF");
    const token = await signToken(sessionClaims({ role: "STAFF", is_admin: false }));
    const response = await worker.fetch(new Request("https://worker.test/api/admin/users", {
      headers: { authorization: `Bearer ${token}` },
    }), makeEnv(db));
    expect(response.status).toBe(403);
    expect(await response.json()).toMatchObject({ error: "FORBIDDEN" });
  });
});

 
describe("Phase 3 WebSocket and Staff file boundaries", () => {
  it("rejects an event WebSocket without the onyx-bearer protocol and access token", async () => {
    const db = makeDb("STAFF");
    const response = await worker.fetch(new Request("https://worker.test/api/events", {
      headers: { upgrade: "websocket", connection: "Upgrade", origin: "https://app.example" },
    }), makeEnv(db));
    expect(response.status).toBe(401);
    expect(await response.json()).toMatchObject({ error: "UNAUTHORIZED" });
  });

  it("does not broaden the existing event-stream role boundary to Staff", async () => {
    const db = makeDb("STAFF");
    const token = await signToken(sessionClaims({ role: "STAFF", is_admin: false }));
    const response = await worker.fetch(new Request("https://worker.test/api/events", {
      headers: {
        upgrade: "websocket",
        connection: "Upgrade",
        origin: "https://app.example",
        "sec-websocket-protocol": "onyx-bearer, " + token,
      },
    }), makeEnv(db));
    expect(response.status).toBe(403);
    expect(await response.json()).toMatchObject({ error: "FORBIDDEN" });
  });

  it("rejects disallowed WebSocket origins before authentication", async () => {
    const db = makeDb("STAFF");
    const response = await worker.fetch(new Request("https://worker.test/api/events", {
      headers: {
        upgrade: "websocket",
        connection: "Upgrade",
        origin: "https://evil.example",
        "sec-websocket-protocol": "onyx-bearer, invalid.token",
      },
    }), makeEnv(db));
    expect(response.status).toBe(403);
    expect(await response.json()).toMatchObject({ error: "ORIGIN_NOT_ALLOWED" });
  });

  it("keeps Staff file downloads fail-closed before looking up an asset", async () => {
    const db = makeDb("STAFF");
    db.fileAssets.set("private-hash", { content_hash: "private-hash", organization_id: ORG });
    const token = await signToken(sessionClaims({ role: "STAFF", is_admin: false }));
    const response = await worker.fetch(new Request("https://worker.test/api/files/private-hash", {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(403);
    expect(await response.json()).toMatchObject({ error: "FORBIDDEN" });
    expect(db.fileAssetLookups).toBe(0);
    expect(db.writes).toHaveLength(0);
  });
});


describe("B1 client-call route contracts", () => {
  it("returns only active same-organization identities from GET /api/users", async () => {
    const db = makeDb("STAFF");
    db.users.set("inactive-staff", {
      id: "inactive-staff", username: "inactive", email: "inactive@example.test",
      organization_id: ORG, is_admin: 0, is_active: 0, class: "staff", role: "STAFF",
    });
    db.users.set("foreign-staff", {
      id: "foreign-staff", username: "foreign", email: "foreign@example.test",
      organization_id: "org-2", is_admin: 0, is_active: 1, class: "staff", role: "STAFF",
    });
    const token = await signToken(sessionClaims({ role: "STAFF", is_admin: false }));
    const response = await worker.fetch(new Request("https://worker.test/api/users", {
      headers: { authorization: "Bearer " + token },
    }), makeEnv(db));
    expect(response.status).toBe(200);
    const users = await response.json() as Array<Record<string, unknown>>;
    expect(users).toEqual([{ id: "staff-1", username: "staff" }]);
    expect(users.every((user) => Object.keys(user).sort().join(",") === "id,username")).toBe(true);
  });

  it("returns a machine-classifiable 501 envelope for an authenticated unported route", async () => {
    const db = makeDb("STAFF");
    const token = await signToken(sessionClaims({ role: "STAFF", is_admin: false }));
    const response = await worker.fetch(new Request("https://worker.test/api/todo/lists", {
      method: "POST",
      headers: { authorization: "Bearer " + token, "content-type": "application/json" },
      body: JSON.stringify({ owner: "staff-1", origin: "staff_authored", items: [] }),
    }), makeEnv(db));
    expect(response.status).toBe(501);
    expect(await response.json()).toMatchObject({
      error: "ROUTE_NOT_MIGRATED",
      route: "/api/todo/lists",
      method: "POST",
      category: "FEATURE_UNAVAILABLE",
      retryability: "NON_RETRYABLE",
    });
    expect((await response.json().catch(() => ({})) as Record<string, unknown>).correlation_id).toBeUndefined();
  });
});
