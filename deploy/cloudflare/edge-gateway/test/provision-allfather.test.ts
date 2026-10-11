/**
 * All-Father provisioning tests (Phase 4 — reports/auth-repair/04-allfather-provisioning.md).
 *
 * Runs the real migration sequence against an in-memory SQLite database (the
 * same engine family as D1) and exercises scripts/provision-allfather.mjs, the
 * only supported path that binds the verified Supabase subject to the one
 * canonical ONYX All-Father principal. Covers:
 *  - idempotence (a second run writes nothing);
 *  - duplicate/ambiguous principal rejection;
 *  - inactive principal rejection (and the explicit --activate path);
 *  - refusal to bind a subject that belongs to another principal, and
 *    refusal to create a principal, rebind, or grant privilege from an
 *    email/username/Google identity;
 *  - guarded writes + audit records, with no credential material anywhere.
 */
import { describe, expect, it } from "vitest";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import { join } from "node:path";

// Loaded via createRequire so Vite's static analysis does not resolve the
// built-in `node:sqlite` module through the ESM resolver (same as migrations.test.ts).
const nodeRequire = createRequire(import.meta.url);
const { DatabaseSync } = nodeRequire("node:sqlite") as typeof import("node:sqlite");
type DatabaseSync = InstanceType<typeof DatabaseSync>;

// @ts-expect-error - plain ESM admin script, intentionally untyped.
import {
  ALLFATHER_PRINCIPAL_ID,
  ALLFATHER_ROLE,
  normalizeOptions,
  parseArgs,
  provisionAllFather,
  readAllFatherState,
  renderSql,
  sqlLiteral,
} from "../scripts/provision-allfather.mjs";

const MIGRATIONS_DIR = fileURLToPath(new URL("../migrations", import.meta.url));
const migrationFiles = readdirSync(MIGRATIONS_DIR)
  .filter((file) => file.endsWith(".sql"))
  .sort();

const SUBJECT_A = "11111111-1111-4111-8111-111111111111";
const SUBJECT_B = "22222222-2222-4222-8222-222222222222";
const OTHER_PRINCIPAL = "99999999-9999-4999-8999-999999999999";

function migratedDb(): DatabaseSync {
  const db = new DatabaseSync(":memory:");
  for (const file of migrationFiles) {
    db.exec(readFileSync(join(MIGRATIONS_DIR, file), "utf8"));
  }
  return db;
}

interface Adapter {
  all(sql: string, params?: unknown[]): Promise<Array<Record<string, unknown>>>;
  run(sql: string, params?: unknown[]): Promise<{ changes?: number }>;
}

function adapter(db: DatabaseSync): Adapter {
  return {
    all: async (sql, params = []) => db.prepare(sql).all(...(params as never[])) as Array<Record<string, unknown>>,
    run: async (sql, params = []) => {
      const result = db.prepare(sql).run(...(params as never[]));
      return { changes: Number(result.changes) };
    },
  };
}

function provisionOptions(overrides: Record<string, unknown> = {}) {
  return { environment: "local", subject: SUBJECT_A, ...overrides };
}

function principalRow(db: DatabaseSync): Record<string, unknown> {
  return db
    .prepare("SELECT * FROM users WHERE id = ?")
    .get(ALLFATHER_PRINCIPAL_ID) as Record<string, unknown>;
}

function auditRows(db: DatabaseSync): Array<Record<string, unknown>> {
  return db
    .prepare("SELECT organization_id, user_id, action, details FROM audit_log ORDER BY id")
    .all() as Array<Record<string, unknown>>;
}

describe("schema precondition", () => {
  it("migrations seed exactly one inert ALL_FATHER principal with no subject mapping", () => {
    const db = migratedDb();
    const rows = db
      .prepare("SELECT id, role, is_active, is_admin, supabase_user_id FROM users")
      .all() as Array<Record<string, unknown>>;
    expect(rows).toHaveLength(1);
    expect(rows[0].id).toBe(ALLFATHER_PRINCIPAL_ID);
    expect(rows[0].role).toBe(ALLFATHER_ROLE);
    expect(Number(rows[0].is_active)).toBe(1);
    expect(Number(rows[0].is_admin)).toBe(1);
    expect(rows[0].supabase_user_id).toBeNull();
  });
});

describe("provisioning", () => {
  it("binds the verified subject, writes one audit record, and never exposes credential material", async () => {
    const db = migratedDb();
    const result = await provisionAllFather(adapter(db), provisionOptions(), { now: () => 1000 });

    expect(result.status).toBe("provisioned");
    expect(principalRow(db).supabase_user_id).toBe(SUBJECT_A);

    const audits = auditRows(db);
    expect(audits).toHaveLength(1);
    expect(audits[0].action).toBe("allfather.provision");
    const details = JSON.parse(String(audits[0].details)) as Record<string, unknown>;
    expect(details).toMatchObject({
      target: ALLFATHER_PRINCIPAL_ID,
      environment: "local",
      previous_subject: null,
      new_subject: SUBJECT_A,
      rebind: false,
    });
    // No credential-shaped fields and no password/hash/token/secret values.
    for (const key of Object.keys(details)) {
      expect(key).not.toMatch(/password|secret|token|hash|key|credential/i);
    }
    expect(String(audits[0].details)).not.toMatch(/\$argon2|\$2[aby]\$|eyJ/);

    const state = await readAllFatherState(adapter(db), SUBJECT_A);
    expect(Object.keys(state.principal ?? {})).not.toContain("password_hash");
  });

  it("is idempotent: re-running writes nothing new", async () => {
    const db = migratedDb();
    await provisionAllFather(adapter(db), provisionOptions(), { now: () => 1000 });
    const result = await provisionAllFather(adapter(db), provisionOptions(), { now: () => 2000 });

    expect(result.status).toBe("already-provisioned");
    expect(result.writes).toHaveLength(0);
    expect(principalRow(db).supabase_user_id).toBe(SUBJECT_A);
    expect(auditRows(db)).toHaveLength(1);
  });

  it("--dry-run performs the preflight and plan without any write", async () => {
    const db = migratedDb();
    const result = await provisionAllFather(
      adapter(db),
      provisionOptions({ dryRun: true }),
      { now: () => 1000 },
    );
    expect(result.status).toBe("provisioned");
    expect(result.dryRun).toBe(true);
    expect(result.plannedWrites.length).toBeGreaterThan(0);
    expect(principalRow(db).supabase_user_id).toBeNull();
    expect(auditRows(db)).toHaveLength(0);
  });
});

describe("fail-closed refusals", () => {
  it("refuses a subject already bound to a different principal (no double-bind)", async () => {
    const db = migratedDb();
    db.prepare(
      "INSERT INTO users(id,username,organization_id,password_hash,is_admin,is_active,class,role,parent_user_id,created_at,updated_at,supabase_user_id) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
    ).run(OTHER_PRINCIPAL, "someone", "__allfather__", "x", 0, 1, "STAFF", "STAFF", null, 1, 1, SUBJECT_A);

    const result = await provisionAllFather(adapter(db), provisionOptions());
    expect(result.status).toBe("refused");
    expect(result.code).toBe("SUBJECT_BOUND_TO_OTHER_PRINCIPAL");
    expect(principalRow(db).supabase_user_id).toBeNull();
    expect(auditRows(db)).toHaveLength(0);
  });

  it("refuses when more than one ALL_FATHER principal exists", async () => {
    const db = migratedDb();
    db.prepare(
      "INSERT INTO users(id,username,organization_id,password_hash,is_admin,is_active,class,role,parent_user_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)",
    ).run(OTHER_PRINCIPAL, "allfather2", "__allfather__", "x", 1, 1, "ALL_FATHER", "ALL_FATHER", null, 1, 1);

    const result = await provisionAllFather(adapter(db), provisionOptions());
    expect(result.code).toBe("PRINCIPAL_AMBIGUOUS");
    expect(auditRows(db)).toHaveLength(0);
  });

  it("refuses to create a principal when the canonical row is missing", async () => {
    const db = migratedDb();
    db.prepare("DELETE FROM users").run();

    const result = await provisionAllFather(adapter(db), provisionOptions());
    expect(result.code).toBe("PRINCIPAL_MISSING");
    expect(db.prepare("SELECT COUNT(*) AS count FROM users").get()).toMatchObject({ count: 0 });
  });

  it("refuses implicit privilege: role must be the explicit ALL_FATHER value", async () => {
    const db = migratedDb();
    // The Worker's roleForUser() falls back to the literal username 'allfather'
    // when role is empty, which would grant privilege implicitly.
    db.prepare("UPDATE users SET role = '' WHERE id = ?").run(ALLFATHER_PRINCIPAL_ID);

    const result = await provisionAllFather(adapter(db), provisionOptions());
    expect(result.code).toBe("PRINCIPAL_ROLE_NOT_EXPLICIT");
    expect(principalRow(db).supabase_user_id).toBeNull();
  });

  it("refuses an inactive principal unless --activate is explicit, then activates and audits", async () => {
    const db = migratedDb();
    db.prepare("UPDATE users SET is_active = 0 WHERE id = ?").run(ALLFATHER_PRINCIPAL_ID);

    const refused = await provisionAllFather(adapter(db), provisionOptions());
    expect(refused.code).toBe("PRINCIPAL_INACTIVE");
    expect(principalRow(db).supabase_user_id).toBeNull();

    const activated = await provisionAllFather(
      adapter(db),
      provisionOptions({ activate: true }),
      { now: () => 1000 },
    );
    expect(activated.status).toBe("provisioned");
    expect(Number(principalRow(db).is_active)).toBe(1);
    expect(principalRow(db).supabase_user_id).toBe(SUBJECT_A);
  });

  it("requires --rebind to replace an existing subject and records the previous value", async () => {
    const db = migratedDb();
    await provisionAllFather(adapter(db), provisionOptions(), { now: () => 1000 });

    const refused = await provisionAllFather(adapter(db), provisionOptions({ subject: SUBJECT_B }));
    expect(refused.code).toBe("REBIND_REQUIRES_APPROVAL");
    expect(principalRow(db).supabase_user_id).toBe(SUBJECT_A);

    const rebound = await provisionAllFather(
      adapter(db),
      provisionOptions({ subject: SUBJECT_B, rebind: true }),
      { now: () => 2000 },
    );
    expect(rebound.status).toBe("provisioned");
    expect(principalRow(db).supabase_user_id).toBe(SUBJECT_B);

    const latest = auditRows(db).at(-1)!;
    expect(JSON.parse(String(latest.details))).toMatchObject({
      previous_subject: SUBJECT_A,
      new_subject: SUBJECT_B,
      rebind: true,
    });
  });

  it("detects a missing auth schema instead of failing mid-write", async () => {
    const db = new DatabaseSync(":memory:");
    db.exec("CREATE TABLE users (id TEXT PRIMARY KEY)");

    const result = await provisionAllFather(adapter(db), provisionOptions());
    expect(result.code).toBe("AUTH_SCHEMA_MISSING");
    expect(String(result.message)).toContain("supabase_user_id");
  });
});

describe("input and environment guards", () => {
  it("rejects malformed subjects, environments, and actions", () => {
    expect(normalizeOptions({ environment: "local", subject: "not-a-uuid" }).refusal?.code).toBe("INVALID_SUBJECT");
    expect(normalizeOptions({ environment: "prod", subject: SUBJECT_A }).refusal?.code).toBe("INVALID_ENVIRONMENT");
    expect(normalizeOptions({ environment: "local", subject: SUBJECT_A, action: "promote" }).refusal?.code).toBe("INVALID_ACTION");
  });

  it("requires an approval value for production that restates the subject", async () => {
    const db = migratedDb();
    const missing = await provisionAllFather(adapter(db), provisionOptions({ environment: "production" }));
    expect(missing.code).toBe("PRODUCTION_APPROVAL_REQUIRED");
    expect(principalRow(db).supabase_user_id).toBeNull();

    const approved = await provisionAllFather(
      adapter(db),
      provisionOptions({ environment: "production", approval: SUBJECT_A }),
      { now: () => 1000 },
    );
    expect(approved.status).toBe("provisioned");
  });

  it("refuses a database identity mismatch", async () => {
    const result = normalizeOptions({
      environment: "local",
      subject: SUBJECT_A,
      database: "onyx-free-db",
      expectDatabase: "some-other-db",
    });
    expect(result.refusal?.code).toBe("DATABASE_MISMATCH");
  });

  it("escapes SQL literals and refuses non-scalar values", () => {
    expect(sqlLiteral("O'Brien")).toBe("'O''Brien'");
    expect(sqlLiteral(null)).toBe("NULL");
    expect(() => sqlLiteral({} as unknown as string)).toThrow();
    expect(renderSql("SELECT ? , ?", ["a'b", 7])).toBe("SELECT 'a''b' , 7");
    expect(() => renderSql("SELECT ?", [])).toThrow();
  });
});

describe("revocation (break-glass)", () => {
  it("clears the mapping with an audit record and is idempotent", async () => {
    const db = migratedDb();
    await provisionAllFather(adapter(db), provisionOptions(), { now: () => 1000 });

    const revoked = await provisionAllFather(
      adapter(db),
      { environment: "local", action: "revoke", rebind: true },
      { now: () => 2000 },
    );
    expect(revoked.status).toBe("revoked");
    expect(principalRow(db).supabase_user_id).toBeNull();
    expect(auditRows(db).at(-1)!.action).toBe("allfather.revoke");

    const again = await provisionAllFather(adapter(db), { environment: "local", action: "revoke" });
    expect(again.status).toBe("already-revoked");
    expect(auditRows(db)).toHaveLength(2);
  });

  it("production revoke requires approval that restates the principal id", () => {
    expect(
      normalizeOptions({ environment: "production", action: "revoke", approval: "wrong" }).refusal?.code,
    ).toBe("PRODUCTION_APPROVAL_REQUIRED");
    expect(
      normalizeOptions({ environment: "production", action: "revoke", approval: ALLFATHER_PRINCIPAL_ID }).options?.action,
    ).toBe("revoke");
  });
});

describe("CLI argument handling (--flag forwarding)", () => {
  it("maps every documented flag onto the option object the command expects", () => {
    const { options } = parseArgs([
      "--environment", "production",
      "--action", "provision",
      "--supabase-user-id", SUBJECT_A,
      "--database", "onyx-free-db",
      "--expect-database", "onyx-free-db",
      "--config", "wrangler.deploy.toml",
      "--operator", "owner@example.test",
      "--approve-production", SUBJECT_A,
      "--dry-run",
      "--json",
    ]);
    expect(options).toMatchObject({
      action: "provision",
      environment: "production",
      subject: SUBJECT_A,
      database: "onyx-free-db",
      expectDatabase: "onyx-free-db",
      config: "wrangler.deploy.toml",
      operator: "owner@example.test",
      approval: SUBJECT_A,
      dryRun: true,
      json: true,
      activate: false,
      rebind: false,
    });
  });

  it("forwards the parsed argv through normalizeOptions unchanged", () => {
    // This is exactly what the CLI's main() does: parseArgs(argv) -> normalizeOptions.
    const { options } = parseArgs([
      "--environment", "local",
      "--supabase-user-id", SUBJECT_A,
      "--activate",
      "--rebind",
    ]);
    const normalized = normalizeOptions(options);
    expect(normalized.refusal).toBeUndefined();
    expect(normalized.options).toMatchObject({
      environment: "local",
      subject: SUBJECT_A,
      activate: true,
      rebind: true,
    });
  });

  it("parses --help and treats a trailing value flag as empty", () => {
    expect(parseArgs(["--help"])).toEqual({ help: true });
    const { options } = parseArgs(["--environment", "local", "--supabase-user-id"]);
    expect(options.subject).toBe("");
  });

  it("does not accept a value that looks like the next flag", () => {
    const { options } = parseArgs(["--environment", "local", "--supabase-user-id", "--rebind"]);
    expect(options.subject).toBe("");
    expect(options.rebind).toBe(true);
  });
});

describe("documented npm command contract", () => {
  it("exposes provision:allfather and the script file it points at", () => {
    const here = fileURLToPath(new URL("..", import.meta.url));
    const pkg = JSON.parse(readFileSync(join(here, "package.json"), "utf8")) as {
      scripts?: Record<string, string>;
    };
    const command = pkg.scripts?.["provision:allfather"];
    expect(command).toBe("node scripts/provision-allfather.mjs");
    // The README documents this alias, so the file it invokes must exist relative
    // to the package directory the npm script runs in.
    const target = command!.replace(/^node\s+/, "");
    expect(existsSync(join(here, target))).toBe(true);
  });
});
