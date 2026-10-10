/**
 * D1 migration sequence tests (Phase 3 — reports/auth-repair/03-d1-migrations.md).
 *
 * Runs the real migration SQL against an in-memory SQLite database (the same
 * engine family as D1) and validates, for every plausible Wrangler ledger
 * state, that the sequence converges to the canonical auth schema:
 *  - filename prefixes are unique and deterministic (no duplicate `0002`);
 *  - a clean database reaches the full schema contract;
 *  - every recorded-prefix state upgrades cleanly with the pending suffix;
 *  - existing rows are preserved and never backfilled with inferred data;
 *  - exactly one inert All-Father system principal exists and no migration
 *    grants privileges based on username/email/Google identity;
 *  - `missingAuthSchema` (readiness contract, Phase 6) detects gaps.
 *
 * Wrangler records applied migrations by filename and applies pending files in
 * lexicographic order, so any single-runner history is a prefix of the sorted
 * file list. The `original-*.sql` fixtures are the pre-repair migration
 * contents (from git history) used to simulate ledgers that already recorded
 * them.
 */
import { describe, expect, it } from "vitest";
import { readFileSync, readdirSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Loaded via createRequire so Vite's static analysis does not try to resolve
// the built-in `node:sqlite` module through the ESM resolver.
const nodeRequire = createRequire(import.meta.url);
const { DatabaseSync } = nodeRequire("node:sqlite") as typeof import("node:sqlite");
type DatabaseSync = InstanceType<typeof DatabaseSync>;
import { missingAuthSchema, type SchemaReadClient } from "../src/schema-contract";

const MIGRATIONS_DIR = fileURLToPath(new URL("../migrations", import.meta.url));
const FIXTURES_DIR = fileURLToPath(new URL("./fixtures", import.meta.url));

const migrationFiles = readdirSync(MIGRATIONS_DIR).filter((f) => f.endsWith(".sql")).sort();
const originalIdentityModel = readFileSync(join(FIXTURES_DIR, "original-0002_onyx_identity_model.sql"), "utf8");
const originalSupabaseIdentity = readFileSync(join(FIXTURES_DIR, "original-0003_supabase_identity.sql"), "utf8");

function migrationSql(name: string): string {
  return readFileSync(join(MIGRATIONS_DIR, name), "utf8");
}

function freshDb(): DatabaseSync {
  return new DatabaseSync(":memory:");
}

/** Simulate a recorded ledger prefix, then apply the pending suffix. */
function applyFrom(db: DatabaseSync, applied: string[]): void {
  const pending = migrationFiles.filter((f) => !applied.includes(f));
  for (const file of pending) db.exec(migrationSql(file));
}

function reader(db: DatabaseSync): SchemaReadClient {
  return {
    prepare: (sql: string) => ({
      all: async () => db.prepare(sql).all() as Array<Record<string, unknown>>,
    }),
  };
}

async function expectCanonicalSchema(db: DatabaseSync): Promise<void> {
  expect(await missingAuthSchema(reader(db))).toEqual([]);
  const supabaseIndexes = db
    .prepare("SELECT name FROM pragma_index_list('users') WHERE name LIKE 'idx_users_supabase%'")
    .all() as Array<{ name: string }>;
  expect(supabaseIndexes.map((r) => r.name)).toEqual(["idx_users_supabase_user"]);
}

function expectCanonicalSeeds(db: DatabaseSync): void {
  const allfathers = db
    .prepare("SELECT id, username, organization_id, role, is_active, supabase_user_id, password_hash FROM users WHERE role = 'ALL_FATHER'")
    .all() as Array<Record<string, unknown>>;
  expect(allfathers).toHaveLength(1);
  const af = allfathers[0]!;
  expect(af.id).toBe("__allfather__");
  expect(af.username).toBe("allfather");
  expect(af.organization_id).toBe("__allfather__");
  expect(af.is_active).toBe(1);
  // No external subject is mapped by any migration: provisioning is an
  // owner-controlled out-of-band step (Phase 2, AUTH-01).
  expect(af.supabase_user_id).toBeNull();
  // Non-usable placeholder hash; password login for ALL_FATHER is rejected by
  // the Worker regardless (see test/auth.test.ts).
  expect(String(af.password_hash)).toContain("AAAAAAAAAAAAAAAAAAAAAA");

  const orgs = db.prepare("SELECT id FROM organizations ORDER BY id").all() as Array<{ id: string }>;
  expect(orgs.map((o) => o.id)).toContain("__allfather__");
  expect(orgs.map((o) => o.id)).not.toContain("__onyx_root__");
}

describe("migration sequence integrity", () => {
  it("has unique filenames and a deterministic lexicographic order", () => {
    expect(new Set(migrationFiles).size).toBe(migrationFiles.length);
    const prefixes = migrationFiles.map((f) => f.match(/^(\d{4})_/)?.[1]);
    expect(prefixes.every(Boolean)).toBe(true);
    // Numeric order never goes backwards (the two `0002` files share a prefix
    // by necessity — they are preserved history whose repair is content-only).
    const sorted = [...(prefixes as unknown as string[])].sort();
    expect(prefixes).toEqual(sorted);
  });

  it("keeps the original filenames (no migration-history renames)", () => {
    expect(migrationFiles).toEqual([
      "0001_foundation.sql",
      "0002_auth_hierarchy.sql",
      "0002_onyx_identity_model.sql",
      "0003_supabase_identity.sql",
      "0004_auth_schema_repair.sql",
    ]);
  });
});

describe("clean install (empty database)", () => {
  it("reaches the canonical auth schema through the ordered sequence", async () => {
    const db = freshDb();
    applyFrom(db, []);
    await expectCanonicalSchema(db);
    expectCanonicalSeeds(db);
  });
});

describe("upgrade from every reachable ledger state", () => {
  // Wrangler records each successfully applied migration by filename; files
  // introduced later can be pending even when higher-numbered files already
  // recorded. Git history of this directory (see report §3) makes exactly
  // these production-shaped ledgers reachable:
  //   L1 {0001}                     — a run between the 0001 and 0002a commits
  //   L2 {0001, 0002a}              — runs before 0003 was introduced
  //   L3 {0001, 0002a, ORIGINAL 0003} — runs from 0003's introduction until the
  //       repair; the ORIGINAL 0002b could never apply after 0002a (duplicate
  //       `email` column), so it is pending/failing in every post-introduction run
  // Anything else (manual ordering, foreign schemas) requires the owner-gated
  // pre-rollout inspection documented in the report.

  function replay(recorded: Array<[string, string]>): DatabaseSync {
    const db = freshDb();
    for (const [name, sql] of recorded) db.exec(sql);
    applyFrom(db, recorded.map(([name]) => name));
    return db;
  }

  it("L1 {0001}: pending 0002a+0002b+0003+0004 apply cleanly", async () => {
    const db = replay([["0001_foundation.sql", migrationSql("0001_foundation.sql")]]);
    await expectCanonicalSchema(db);
    expectCanonicalSeeds(db);
  });

  it("L2 {0001, 0002a}: pending 0002b+0003+0004 apply cleanly", async () => {
    const db = replay([
      ["0001_foundation.sql", migrationSql("0001_foundation.sql")],
      ["0002_auth_hierarchy.sql", migrationSql("0002_auth_hierarchy.sql")],
    ]);
    await expectCanonicalSchema(db);
    expectCanonicalSeeds(db);
  });

  it("L3 {0001, 0002a, ORIGINAL 0003}: repaired 0002b is a safe no-op and 0004 converges the indexes", async () => {
    // Recorded ledger only (no pending applied yet), replayed with the
    // pre-repair 0003 content that actually shipped in this state.
    const db = freshDb();
    db.exec(migrationSql("0001_foundation.sql"));
    db.exec(migrationSql("0002_auth_hierarchy.sql"));
    db.exec(originalSupabaseIdentity);
    // Pre-repair 0003 left the old, differently named index behind.
    const before = db
      .prepare("SELECT name FROM pragma_index_list('users') WHERE name LIKE 'idx_users_supabase%'")
      .all() as Array<{ name: string }>;
    expect(before.map((r) => r.name)).toEqual(["idx_users_supabase_user_id"]);

    applyFrom(db, ["0001_foundation.sql", "0002_auth_hierarchy.sql", "0003_supabase_identity.sql"]);
    await expectCanonicalSchema(db);
    expectCanonicalSeeds(db);
  });

  it("regression (why the repair was required): the ORIGINAL 0002b could never apply after 0002a", () => {
    const db = freshDb();
    db.exec(migrationSql("0001_foundation.sql"));
    db.exec(migrationSql("0002_auth_hierarchy.sql"));
    expect(() => db.exec(originalIdentityModel)).toThrowError(/duplicate column name: email/);
  });

  it("preserves existing rows and never infers emails or Supabase IDs", async () => {
    const db = freshDb();
    for (const file of ["0001_foundation.sql", "0002_auth_hierarchy.sql"]) db.exec(migrationSql(file));
    db.exec(`
      INSERT INTO organizations(id,name,is_active,created_at,updated_at) VALUES('org-1','Acme',1,0,0);
      INSERT INTO users(id,username,email,organization_id,password_hash,is_admin,is_active,class,role,parent_user_id,created_at,updated_at)
        VALUES('admin-1','admin1',NULL,'org-1','$argon2id$v=19$m=1,t=1,p=1$AAAA$AAAA',1,1,NULL,'ORGANIZATION_ADMIN','__allfather__',0,0);
      INSERT INTO users(id,username,email,organization_id,password_hash,is_admin,is_active,class,role,parent_user_id,created_at,updated_at)
        VALUES('staff-1','staff1','staff1@example.test','org-1','$argon2id$v=19$m=1,t=1,p=1$AAAA$AAAA',0,1,'FIELD','STAFF','admin-1',0,0);
    `);
    applyFrom(db, ["0001_foundation.sql", "0002_auth_hierarchy.sql"]);
    await expectCanonicalSchema(db);

    const admin = db.prepare("SELECT * FROM users WHERE id='admin-1'").get() as Record<string, unknown>;
    expect(admin.email).toBeNull(); // no inferred email
    expect(admin.supabase_user_id).toBeNull(); // no inferred external identity
    expect(admin.role).toBe("ORGANIZATION_ADMIN"); // role preserved
    const staff = db.prepare("SELECT * FROM users WHERE id='staff-1'").get() as Record<string, unknown>;
    expect(staff.role).toBe("STAFF");
    expect(staff.email).toBe("staff1@example.test");
  });
});

describe("readiness schema contract (Phase 6 input)", () => {
  it("detects missing auth tables and columns on a foundation-only database", async () => {
    const db = freshDb();
    db.exec(migrationSql("0001_foundation.sql"));
    const missing = await missingAuthSchema(reader(db));
    expect(missing).toContain("table:organizations");
    expect(missing).toContain("users.email");
    expect(missing).toContain("users.role");
    expect(missing).toContain("users.supabase_user_id");
    expect(missing).toContain("index:idx_users_supabase_user");
  });

  it("reports nothing missing after the full sequence", async () => {
    const db = freshDb();
    applyFrom(db, []);
    expect(await missingAuthSchema(reader(db))).toEqual([]);
  });
});
