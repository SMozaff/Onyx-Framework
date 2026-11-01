/**
 * Backend accounts lane: end-to-end tests through the Worker fetch handler,
 * against the real migrations on SQLite (test/helpers/d1-sqlite.ts).
 *
 * Owner rules: docs/migrations/slint/backend-accounts-handover.md.
 * Decisions O-1 to O-6: docs/migrations/slint/decision-log.md.
 *
 * Fixtures are generated here. No real names, emails, passwords or tokens.
 * Passwords are random per run and are never printed or asserted by value
 * outside these tests.
 */
import { afterEach, describe, expect, it, vi } from "vitest";
import worker from "../src/index";
import { buildXlsx, readFirstSheet } from "../src/xlsx";
import { tehranGregorianYear } from "../src/staff-rules";
import { fixturePassword, openMigratedDb, seedOrganization, seedUser, type SqliteD1 } from "./helpers/d1-sqlite";

type WorkerEnv = Parameters<typeof worker.fetch>[1];

const ORG = "org-a";
const OTHER_ORG = "org-b";
const XLSX = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
const HEADER = ["onyx_id", "first_name", "last_name", "national_id", "contact_email", "contact_phone", "job_title", "department", "class_label", "parent_onyx_id", "team_ids"];
const SPEC_COLUMNS = [
  "onyx_id", "first_name", "last_name", "first_name_fa", "last_name_fa", "national_id",
  "contact_email", "contact_phone", "job_title", "department", "class_label", "parent_onyx_id",
  "team_ids", "account_status", "one_time_password",
];
const FIRST = ["Alpha", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel"];
const LAST = ["Anders", "Brooks", "Carver", "Dunn", "Ellis", "Frost", "Gale", "Hale"];
const TIMEOUT = 60_000;

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

function envFor(db: SqliteD1): WorkerEnv {
  return {
    DB: db,
    ONYX_JWT_SECRET: "test-only-secret-" + crypto.randomUUID(),
    ONYX_CORS_ORIGINS: "https://app.example",
  } as unknown as WorkerEnv;
}

async function send(env: WorkerEnv, method: string, path: string, options: {
  token?: string; body?: unknown; raw?: BodyInit; contentType?: string;
} = {}): Promise<Response> {
  const headers = new Headers();
  if (options.token) headers.set("authorization", `Bearer ${options.token}`);
  let body: BodyInit | undefined;
  if (options.raw !== undefined) {
    body = options.raw;
    if (options.contentType) headers.set("content-type", options.contentType);
  } else if (options.body !== undefined) {
    body = JSON.stringify(options.body);
    headers.set("content-type", "application/json");
  }
  return worker.fetch(new Request(`https://worker.test${path}`, { method, headers, body }), env);
}

async function signIn(env: WorkerEnv, identifier: string, password: string): Promise<{ status: number; body: Record<string, any> }> {
  const response = await send(env, "POST", "/api/auth/login", { body: { username: identifier, password } });
  return { status: response.status, body: await response.json() as Record<string, any> };
}

interface World {
  db: SqliteD1;
  env: WorkerEnv;
  adminToken: string;
  staffToken: string;
  adminPassword: string;
  staffPassword: string;
}

async function world(): Promise<World> {
  const db = openMigratedDb();
  await seedOrganization(db, ORG);
  await seedOrganization(db, OTHER_ORG);
  const adminPassword = fixturePassword();
  const staffPassword = fixturePassword();
  await seedUser(db, { id: "admin-a", username: "admin-a", organizationId: ORG, role: "ORGANIZATION_ADMIN", password: adminPassword });
  await seedUser(db, { id: "staff-a", username: "staff-a", organizationId: ORG, role: "STAFF", password: staffPassword });
  const env = envFor(db);
  const adminLogin = await signIn(env, "admin-a", adminPassword);
  const staffLogin = await signIn(env, "staff-a", staffPassword);
  return {
    db, env, adminPassword, staffPassword,
    adminToken: adminLogin.body.access_token as string,
    staffToken: staffLogin.body.access_token as string,
  };
}

/** Generated staff row. National IDs are synthetic 10-digit strings. */
function person(index: number, overrides: Record<string, string | { text: string; numeric: true }> = {}) {
  return {
    onyx_id: "",
    first_name: FIRST[index % FIRST.length],
    last_name: LAST[index % LAST.length],
    national_id: `00${String(10000000 + index)}`,
    contact_email: `staff${index}@example.test`,
    contact_phone: `+1555000${String(1000 + index)}`,
    job_title: "Analyst",
    department: "Operations",
    class_label: "Group A",
    parent_onyx_id: "",
    team_ids: "",
    ...overrides,
  } as Record<string, string | { text: string; numeric: true }>;
}

function staffSheet(rows: Array<Record<string, string | { text: string; numeric: true }>>): Uint8Array {
  return buildXlsx("Staff", [HEADER, ...rows.map((row) => HEADER.map((key) => row[key] ?? ""))]);
}

async function importSheet(world: World, rows: Array<Record<string, string | { text: string; numeric: true }>>): Promise<Response> {
  return send(world.env, "POST", "/api/admin/profiles/import", { token: world.adminToken, raw: staffSheet(rows), contentType: XLSX });
}

async function readReport(response: Response): Promise<{ headers: string[]; rows: Array<Record<string, string>> }> {
  const sheet = await readFirstSheet(new Uint8Array(await response.arrayBuffer()));
  return {
    headers: sheet.headers,
    rows: sheet.rows.map((row) => Object.fromEntries(Object.entries(row.cells).map(([key, cell]) => [key, cell.text]))),
  };
}

function yearPrefix(): string {
  return `st${String(tehranGregorianYear() - 2000).padStart(2, "0")}`;
}

describe("backend accounts: import creates accounts (rule 1)", () => {
  it("[O-6] batch import creates one account per staff row", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const response = await importSheet(w, [person(0), person(1), person(2)]);
    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toBe(XLSX);
    expect(response.headers.get("x-onyx-import-created")).toBe("3");
    const report = await readReport(response);
    expect(report.rows).toHaveLength(3);
    for (const row of report.rows) {
      expect(row.account_status).toBe("created");
      expect(row.onyx_id).toMatch(new RegExp(`^${yearPrefix()}[a-z]{2}\\d{4}$`));
    }
    const staff = w.db.rows<{ id: string; username: string; onyx_id: string; must_change_password: number; role: string }>(
      "SELECT id,username,onyx_id,must_change_password,role FROM users WHERE organization_id=? AND onyx_id IS NOT NULL ORDER BY onyx_id", ORG);
    expect(staff.map((u) => u.onyx_id)).toEqual(report.rows.map((r) => r.onyx_id).sort());
    for (const user of staff) {
      expect(user.role).toBe("STAFF");
      expect(user.username).toBe(user.onyx_id);
      expect(user.must_change_password).toBe(1);
    }
    expect(w.db.scalar("SELECT COUNT(*) FROM aggregates WHERE organization_id=? AND aggregate_type='staff_profile'", ORG)).toBe(3);
    expect(w.db.scalar("SELECT COUNT(*) FROM public_ids WHERE organization_id=? AND kind='staff'", ORG)).toBe(3);
  });

  it("[O-5] a row with a blank onyx_id creates account and ID together; a supplied onyx_id is ignored and reported", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const response = await importSheet(w, [
      person(0),
      person(1, { onyx_id: "st99zz9999" }),
    ]);
    expect(response.status).toBe(200);
    const report = await readReport(response);
    expect(report.rows.every((r) => r.account_status === "created")).toBe(true);
    const supplied = report.rows[1];
    expect(supplied.onyx_id).not.toBe("st99zz9999");
    expect(supplied.onyx_id).toMatch(new RegExp(`^${yearPrefix()}`));
    expect(supplied.row_notes).toContain("supplied onyx_id was ignored");
    expect(w.db.scalar("SELECT COUNT(*) FROM users WHERE username='st99zz9999'")).toBe(0);
  });

  it("[unblocked] a failed row leaves no account, no profile and no ID gap", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const response = await importSheet(w, [
      person(0),
      person(1, { first_name: "Invalidname", national_id: "123" }),
      person(2),
    ]);
    const report = await readReport(response);
    expect(report.rows.map((r) => r.account_status)).toEqual(["created", "failed: INVALID_STAFF_ROW", "created"]);
    expect(report.rows[1].onyx_id).toBe("");
    expect(report.rows[1].one_time_password).toBe("");
    const ids = [report.rows[0].onyx_id, report.rows[2].onyx_id];
    // Orders are consecutive: the failed row consumed no number.
    expect(ids.map((id) => id.slice(-4))).toEqual(["0001", "0002"]);
    expect(w.db.scalar("SELECT COUNT(*) FROM users WHERE organization_id=? AND onyx_id IS NOT NULL", ORG)).toBe(2);
    expect(w.db.scalar("SELECT COUNT(*) FROM aggregates WHERE organization_id=? AND aggregate_type='staff_profile'", ORG)).toBe(2);
    expect(w.db.scalar("SELECT COUNT(*) FROM public_ids WHERE organization_id=? AND kind='staff'", ORG)).toBe(2);
    expect(w.db.scalar("SELECT COUNT(*) FROM aggregates WHERE state LIKE '%Invalidname%'")).toBe(0);
  });

  it("[unblocked] an account conflict rolls the whole row back, and the next row takes the next free number", { timeout: TIMEOUT }, async () => {
    const w = await world();
    // Alpha Brooks would receive st<YY>ab0001. Occupy that username in another organization.
    await seedUser(w.db, { id: "other-1", username: `${yearPrefix()}ab0001`, organizationId: OTHER_ORG, role: "STAFF", password: fixturePassword() });
    const response = await importSheet(w, [person(0, { last_name: "Brooks" }), person(2)]); // Alpha Brooks, Charlie Carver
    const report = await readReport(response);
    expect(report.rows[0].account_status).toBe("failed: ACCOUNT_ID_CONFLICT");
    expect(report.rows[1].account_status).toBe("created");
    expect(report.rows[1].onyx_id).toBe(`${yearPrefix()}cc0001`);
    expect(w.db.scalar("SELECT COUNT(*) FROM aggregates WHERE organization_id=? AND aggregate_type='staff_profile'", ORG)).toBe(1);
  });
});

describe("backend accounts: single creation follows the same rule (rule 1)", () => {
  it("[unblocked] POST /api/admin/users creates an account and assigns a staff ID", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const { first_name, last_name, national_id, contact_email, job_title, department, class_label } = person(3);
    const response = await send(w.env, "POST", "/api/admin/users", {
      token: w.adminToken, body: { first_name, last_name, national_id, contact_email, job_title, department, class_label },
    });
    expect(response.status).toBe(201);
    expect(response.headers.get("content-type")).toBe(XLSX);
    const report = await readReport(response);
    expect(report.rows).toHaveLength(1);
    const onyxId = report.rows[0].onyx_id;
    expect(onyxId).toMatch(new RegExp(`^${yearPrefix()}[a-z]{2}\\d{4}$`));
    expect(report.rows[0].account_status).toBe("created");
    expect(report.rows[0].one_time_password.length).toBeGreaterThanOrEqual(12);
    expect(w.db.scalar("SELECT COUNT(*) FROM users WHERE onyx_id=? AND organization_id=? AND role='STAFF'", onyxId, ORG)).toBe(1);
  });

  it("[unblocked] POST /api/admin/users rejects a request that supplies a password", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const supplied = `Supplied-${crypto.randomUUID()}`;
    const response = await send(w.env, "POST", "/api/admin/users", {
      token: w.adminToken, body: { ...person(4), password: supplied },
    });
    const text = await response.text();
    expect(response.status).toBe(400);
    expect(text).toContain("INVALID_BODY");
    expect(text).not.toContain(supplied);
    expect(w.db.scalar("SELECT COUNT(*) FROM users WHERE onyx_id IS NOT NULL")).toBe(0);
  });
});

describe("backend accounts: sign-in with Onyx ID (rule 2)", () => {
  it("[O-3] login accepts the Onyx ID with the one-time password", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0)]));
    const { onyx_id: onyxId, one_time_password: password } = report.rows[0];
    const login = await signIn(w.env, onyxId.toUpperCase(), password);
    expect(login.status).toBe(200);
    expect(login.body.user.onyx_id).toBe(onyxId);
    expect(login.body.user.must_change_password).toBe(true);
    expect(login.body.user.role).toBe("STAFF");
  });

  it("[O-3] username and email remain accepted as aliases during transition", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(5)]));
    const { one_time_password: password, contact_email: email } = report.rows[0];
    expect((await signIn(w.env, email, password)).status).toBe(200);
    expect((await signIn(w.env, "admin-a", w.adminPassword)).status).toBe(200);
    expect((await signIn(w.env, "staff-a", "wrong-" + crypto.randomUUID())).status).toBe(401);
  });
});

describe("backend accounts: one-time password (rule 3)", () => {
  it("[O-1] the generated password meets the 12-character minimum and uses an unambiguous alphabet", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0)]));
    const password = report.rows[0].one_time_password;
    expect(password.length).toBeGreaterThanOrEqual(12);
    expect(password).toMatch(/^[A-HJ-NP-Za-km-z2-9]+$/);
  });

  it("[unblocked] every new account gets a distinct generated password", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0), person(1), person(2), person(3)]));
    const passwords = report.rows.map((r) => r.one_time_password);
    expect(new Set(passwords).size).toBe(4);
  });
});

describe("backend accounts: forced change at first login (rule 4)", () => {
  it("[O-4] a new account cannot call any route except password change until the password is changed", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0)]));
    const login = await signIn(w.env, report.rows[0].onyx_id, report.rows[0].one_time_password);
    const token = login.body.access_token as string;
    for (const [method, path] of [["GET", "/api/profiles"], ["GET", "/api/query"], ["POST", "/api/command"], ["POST", "/api/relay-ticket"]] as const) {
      const response = await send(w.env, method, path, { token, body: method === "POST" ? {} : undefined });
      expect(response.status, `${method} ${path}`).toBe(409);
      expect(await response.json()).toMatchObject({ error: "PASSWORD_CHANGE_REQUIRED" });
    }
    // Logout is still allowed; password change is reachable (and checks the current password).
    const wrong = await send(w.env, "POST", "/api/auth/password", { token, body: { current_password: "not-it-" + crypto.randomUUID(), new_password: fixturePassword() } });
    expect(wrong.status).toBe(403);
    const logout = await send(w.env, "POST", "/api/auth/logout", { token, body: {} });
    expect(logout.status).toBe(200);
  });

  it("[O-4] after a password change the forced-change flag is cleared and other routes open", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0)]));
    const { onyx_id: onyxId, one_time_password: oneTime } = report.rows[0];
    const first = await signIn(w.env, onyxId, oneTime);
    const newPassword = fixturePassword();
    const change = await send(w.env, "POST", "/api/auth/password", {
      token: first.body.access_token, body: { current_password: oneTime, new_password: newPassword },
    });
    expect(change.status).toBe(200);
    expect(await change.json()).toMatchObject({ success: true, reauthentication_required: true });
    expect(w.db.scalar("SELECT must_change_password FROM users WHERE onyx_id=?", onyxId)).toBe(0);

    // Sessions are revoked by the change, so the old token stops working.
    const stale = await send(w.env, "GET", "/api/profiles", { token: first.body.access_token });
    expect(stale.status).toBe(401);

    const second = await signIn(w.env, onyxId, newPassword);
    expect(second.status).toBe(200);
    expect(second.body.user.must_change_password).toBe(false);
    const open = await send(w.env, "GET", "/api/profiles", { token: second.body.access_token });
    expect(open.status).toBe(200);
  });

  it("[unblocked] the password change enforces the 12-character minimum", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0)]));
    const login = await signIn(w.env, report.rows[0].onyx_id, report.rows[0].one_time_password);
    const response = await send(w.env, "POST", "/api/auth/password", {
      token: login.body.access_token, body: { current_password: report.rows[0].one_time_password, new_password: "short-pass1" },
    });
    expect(response.status).toBe(400);
    expect(await response.json()).toMatchObject({ error: "WEAK_PASSWORD" });
    expect(w.db.scalar("SELECT must_change_password FROM users WHERE onyx_id=?", report.rows[0].onyx_id)).toBe(1);
  });
});

describe("backend accounts: password handling (rule 5)", () => {
  it("[O-1] only a hash is stored for each account", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0), person(1)]));
    const passwords = report.rows.map((r) => r.one_time_password);
    const hashes = w.db.rows<{ password_hash: string }>("SELECT password_hash FROM users WHERE onyx_id IS NOT NULL");
    expect(hashes).toHaveLength(2);
    for (const { password_hash } of hashes) expect(password_hash.startsWith("$argon2id$")).toBe(true);
    // No table anywhere in the database holds a generated password in plain text.
    const tables = w.db.rows<{ name: string }>("SELECT name FROM sqlite_master WHERE type='table'");
    for (const { name } of tables) {
      const dump = JSON.stringify(w.db.rows(`SELECT * FROM "${name}"`));
      for (const password of passwords) expect(dump, `table ${name}`).not.toContain(password);
    }
  });

  it("[unblocked] no response from login or profile reads contains a generated password", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0)]));
    const { onyx_id: onyxId, one_time_password: password } = report.rows[0];
    const login = await send(w.env, "POST", "/api/auth/login", { body: { username: onyxId, password } });
    expect(await login.text()).not.toContain(password);
    const profiles = await send(w.env, "GET", "/api/profiles", { token: w.adminToken });
    expect(await profiles.text()).not.toContain(password);
  });

  it("[unblocked] no log line from import, create or login contains a password or a hash", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const logged: string[] = [];
    for (const method of ["log", "info", "warn", "error", "debug"] as const) {
      vi.spyOn(console, method).mockImplementation((...args: unknown[]) => { logged.push(args.map(String).join(" ")); });
    }
    const report = await readReport(await importSheet(w, [person(0)]));
    const created = await send(w.env, "POST", "/api/admin/users", { token: w.adminToken, body: person(6) });
    const createdReport = await readReport(created);
    await signIn(w.env, report.rows[0].onyx_id, report.rows[0].one_time_password);
    await signIn(w.env, report.rows[0].onyx_id, "wrong-" + crypto.randomUUID());
    const secrets = [...report.rows, ...createdReport.rows].map((r) => r.one_time_password);
    const joined = logged.join("\n");
    for (const secret of secrets) expect(joined).not.toContain(secret);
    expect(joined).not.toContain("$argon2id$");
  });

  it("[unblocked] import and create request bodies are not echoed in errors", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const secret = `Echo-${crypto.randomUUID()}`;
    const csv = `first_name,last_name,password,national_id\r\nAlpha,Anders,${secret},0011223344\r\n`;
    const importResponse = await send(w.env, "POST", "/api/admin/profiles/import", { token: w.adminToken, raw: csv, contentType: "text/csv" });
    const importText = await importResponse.text();
    expect(importResponse.status).toBe(400);
    expect(importText).not.toContain(secret);
    const createResponse = await send(w.env, "POST", "/api/admin/users", { token: w.adminToken, body: { ...person(7), password: secret } });
    expect(await createResponse.text()).not.toContain(secret);
  });
});

describe("backend accounts: creation report (rule 6)", () => {
  it("[O-4] the report has the columns in the creation report spec, then source_row and row_notes", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0)]));
    expect(report.headers).toEqual([...SPEC_COLUMNS, "source_row", "row_notes"]);
  });

  it("[unblocked] the report is produced after every batch import", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const response = await importSheet(w, [person(0), person(1)]);
    expect(response.headers.get("content-type")).toBe(XLSX);
    expect(response.headers.get("content-disposition")).toContain("attachment");
    expect(response.headers.get("cache-control")).toBe("no-store");
    expect(response.headers.get("x-onyx-import-total")).toBe("2");
    expect((await readReport(response)).rows).toHaveLength(2);
  });

  it("[unblocked] the report is produced after every single creation", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const response = await send(w.env, "POST", "/api/admin/users", { token: w.adminToken, body: person(0) });
    expect(response.status).toBe(201);
    expect(response.headers.get("content-type")).toBe(XLSX);
    expect((await readReport(response)).rows).toHaveLength(1);
  });

  it("[unblocked] text cells starting with = + - @ are written as text", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const response = await importSheet(w, [
      person(0, { job_title: "=HYPERLINK(\"https://example.test\")", department: "@SUM(1,1)", class_label: "-2+3", contact_phone: "+1555000999" }),
    ]);
    const row = (await readReport(response)).rows[0];
    expect(row.job_title).toBe("'=HYPERLINK(\"https://example.test\")");
    expect(row.department).toBe("'@SUM(1,1)");
    expect(row.class_label).toBe("'-2+3");
    expect(row.contact_phone).toBe("'+1555000999");
  });

  it("[unblocked] national_id keeps its leading zeros in the report", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const row = (await readReport(await importSheet(w, [person(0, { national_id: "0012345678" })]))).rows[0];
    expect(row.national_id).toBe("0012345678");
  });

  it("[unblocked] a blank contact_email uses the placeholder and is flagged in the report", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const row = (await readReport(await importSheet(w, [person(0, { contact_email: "" })]))).rows[0];
    expect(row.contact_email).toBe(`${row.onyx_id}@accounts.invalid`);
    expect(row.row_notes).toContain("placeholder");
    expect(w.db.scalar("SELECT email FROM users WHERE onyx_id=?", row.onyx_id)).toBe(`${row.onyx_id}@accounts.invalid`);
  });
});

describe("backend accounts: IDs (rule 7)", () => {
  it("[O-2] public_ids and id_sequences exist in migrations/ and D1 is the account store", () => {
    const w = openMigratedDb();
    const tables = w.rows<{ name: string }>("SELECT name FROM sqlite_master WHERE type='table'").map((t) => t.name);
    expect(tables).toEqual(expect.arrayContaining(["id_sequences", "public_ids"]));
    const columns = w.rows<{ name: string }>("PRAGMA table_info(users)").map((c) => c.name);
    expect(columns).toEqual(expect.arrayContaining(["onyx_id", "must_change_password"]));
  });

  it("[unblocked] staff IDs are unique per organization and year, and never reused after removal", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const otherAdminPassword = fixturePassword();
    await seedUser(w.db, { id: "admin-b", username: "admin-b", organizationId: OTHER_ORG, role: "ORGANIZATION_ADMIN", password: otherAdminPassword });
    const otherToken = (await signIn(w.env, "admin-b", otherAdminPassword)).body.access_token as string;

    // Each organization numbers from 0001 for its own year.
    const otherOrgFirst = await send(w.env, "POST", "/api/admin/profiles/import", {
      token: otherToken, raw: staffSheet([person(4)]), contentType: XLSX,
    });
    const otherId = (await readReport(otherOrgFirst)).rows[0].onyx_id;
    expect(otherId.slice(-4)).toBe("0001");

    const first = (await readReport(await importSheet(w, [person(0)]))).rows[0].onyx_id;
    const removal = await send(w.env, "DELETE", `/api/admin/users/${w.db.scalar("SELECT id FROM users WHERE onyx_id=?", first)}`, { token: w.adminToken });
    expect(removal.status).toBe(200);
    // A new person (new email) after removal takes the next number, not the freed one.
    const second = (await readReport(await importSheet(w, [person(1)]))).rows[0].onyx_id;
    expect(second.slice(-4)).toBe("0002");
    expect(w.db.scalar("SELECT COUNT(*) FROM public_ids WHERE public_id=?", first)).toBe(1);
    expect(w.db.scalar("SELECT COUNT(DISTINCT public_id) FROM public_ids WHERE organization_id=?", ORG)).toBe(2);
  });

  it("[unblocked] concurrent allocations in one organization and year produce distinct, gapless numbers", { timeout: TIMEOUT }, async () => {
    const w = await world();
    // Same initials for every request, so the order numbers are the only difference.
    const responses = await Promise.all(Array.from({ length: 6 }, (_, i) =>
      send(w.env, "POST", "/api/admin/users", { token: w.adminToken, body: person(0, { contact_email: `concurrent${i}@example.test`, national_id: `00${String(20000000 + i)}` }) })));
    expect(responses.map((r) => r.status)).toEqual(Array(6).fill(201));
    const ids = (await Promise.all(responses.map(async (r) => (await readReport(r)).rows[0].onyx_id))).sort();
    expect(new Set(ids).size).toBe(6);
    expect(ids.map((id) => Number(id.slice(-4)))).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it("[unblocked] the per-year counter restarts at 0001 on the Asia/Tehran year boundary", { timeout: TIMEOUT }, async () => {
    // 2026-12-31 20:29 UTC is still 2026 in Tehran. 20:31 UTC is 2027-01-01 00:01 in Tehran.
    // The clock is set before sign-in so access tokens are valid across both imports.
    vi.useFakeTimers({ toFake: ["Date"] });
    vi.setSystemTime(new Date("2026-12-31T20:29:00Z"));
    const w = await world();
    const before = (await readReport(await importSheet(w, [person(0)]))).rows[0].onyx_id;
    vi.setSystemTime(new Date("2026-12-31T20:31:00Z"));
    const after = (await readReport(await importSheet(w, [person(1)]))).rows[0].onyx_id;
    expect(before.slice(0, 4)).toBe("st26");
    expect(after.slice(0, 4)).toBe("st27");
    expect(after.slice(-4)).toBe("0001");
  });

  it("[unblocked] a sequence past 9999 fails the row and writes nothing", { timeout: TIMEOUT }, async () => {
    const w = await world();
    w.db.sqlite.prepare("INSERT INTO id_sequences(organization_id,kind,year,last_value) VALUES(?,'staff',?,9999)").run(ORG, tehranGregorianYear());
    const row = (await readReport(await importSheet(w, [person(0)]))).rows[0];
    expect(row.account_status).toBe("failed: ID_SEQUENCE_EXHAUSTED");
    expect(w.db.scalar("SELECT COUNT(*) FROM users WHERE onyx_id IS NOT NULL")).toBe(0);
    expect(w.db.scalar("SELECT COUNT(*) FROM aggregates WHERE aggregate_type='staff_profile'")).toBe(0);
  });
});

describe("backend accounts: import formats, roles and references", () => {
  it("CSV with a byte-order mark and quoted fields creates accounts", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const csv = "﻿first_name,last_name,national_id,contact_email,job_title,onyx_id,department,class_label,contact_phone,parent_onyx_id,team_ids\r\n" +
      "Echo,\"Ellis, Jr\",0055512345,echo@example.test,\"Lead, Ops\",,Ops,A,+15550002222,,\r\n";
    const response = await send(w.env, "POST", "/api/admin/profiles/import", { token: w.adminToken, raw: csv, contentType: "text/csv" });
    expect(response.status).toBe(200);
    const row = (await readReport(response)).rows[0];
    expect(row.account_status).toBe("created");
    expect(row.last_name).toBe("Ellis, Jr");
    expect(row.job_title).toBe("Lead, Ops");
  });

  it("JSON array import creates accounts", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const rows = [person(0), person(1)].map(({ onyx_id: _ignored, ...rest }) => rest);
    const response = await send(w.env, "POST", "/api/admin/profiles/import", { token: w.adminToken, body: rows });
    expect(response.status).toBe(200);
    expect(response.headers.get("x-onyx-import-created")).toBe("2");
  });

  it("a CSV without the national_id column is rejected before anything is created", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const response = await send(w.env, "POST", "/api/admin/profiles/import", {
      token: w.adminToken, raw: "onyx_id,first_name,last_name\r\n,Alpha,Anders\r\n", contentType: "text/csv",
    });
    expect(response.status).toBe(400);
    expect(await response.json()).toMatchObject({ error: "IMPORT_MISSING_COLUMN", column: "national_id" });
    expect(w.db.scalar("SELECT COUNT(*) FROM users WHERE onyx_id IS NOT NULL")).toBe(0);
  });

  it("a national_id stored as a number is rejected, not guessed", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const row = (await readReport(await importSheet(w, [person(0, { national_id: { text: "0012345678", numeric: true } })]))).rows[0];
    expect(row.account_status).toBe("failed: INVALID_STAFF_ROW");
    expect(row.row_notes).toContain("stored as text");
  });

  it("refuses unreadable, non-array, oversized and over-long imports before anything is created", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const brokenWorkbook = new Uint8Array([0x50, 0x4b, 0x03, 0x04, 0x00, 0x01, 0x02]);
    const broken = await send(w.env, "POST", "/api/admin/profiles/import", { token: w.adminToken, raw: brokenWorkbook, contentType: XLSX });
    expect(broken.status).toBe(400);
    expect(await broken.json()).toMatchObject({ error: "INVALID_PROFILE_IMPORT" });

    const notArray = await send(w.env, "POST", "/api/admin/profiles/import", { token: w.adminToken, body: { first_name: "Alpha" } });
    expect(notArray.status).toBe(400);
    expect(await notArray.json()).toMatchObject({ error: "INVALID_PROFILE_IMPORT" });

    const tooLarge = await send(w.env, "POST", "/api/admin/profiles/import", { token: w.adminToken, raw: "x".repeat(2_000_001), contentType: "text/csv" });
    expect(tooLarge.status).toBe(413);

    // 501 rows with a blank name: the cap applies to non-empty rows, before any row is processed.
    const over = staffSheet(Array.from({ length: 501 }, (_, i) => person(i % 8, { first_name: "", contact_email: `over${i}@example.test` })));
    const overResponse = await send(w.env, "POST", "/api/admin/profiles/import", { token: w.adminToken, raw: over, contentType: XLSX });
    expect(overResponse.status).toBe(400);
    expect(await overResponse.json()).toMatchObject({ error: "IMPORT_TOO_MANY_ROWS" });

    expect(w.db.scalar("SELECT COUNT(*) FROM users WHERE onyx_id IS NOT NULL")).toBe(0);
  });

  it("only an Admin can import or create staff", { timeout: TIMEOUT }, async () => {
    const w = await world();
    expect((await importSheet({ ...w, adminToken: w.staffToken }, [person(0)])).status).toBe(403);
    expect((await send(w.env, "POST", "/api/admin/users", { token: w.staffToken, body: person(1) })).status).toBe(403);
  });

  it("parent_onyx_id links to an active account in the organization; an unknown ID fails the row", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const manager = (await readReport(await send(w.env, "POST", "/api/admin/users", { token: w.adminToken, body: person(0) }))).rows[0].onyx_id;
    const report = await readReport(await importSheet(w, [person(1, { parent_onyx_id: manager.toUpperCase() }), person(2, { parent_onyx_id: "st00zz0000" })]));
    expect(report.rows.map((r) => r.account_status)).toEqual(["created", "failed: PARENT_NOT_FOUND"]);
    const child = await w.db.prepare("SELECT parent_user_id FROM users WHERE onyx_id=?").bind(report.rows[0].onyx_id).first<{ parent_user_id: string }>();
    expect(child?.parent_user_id).toBe(w.db.scalar("SELECT id FROM users WHERE onyx_id=?", manager));
  });

  it("a row missing required fields reports every problem and the rest of the file still imports", { timeout: TIMEOUT }, async () => {
    const w = await world();
    const report = await readReport(await importSheet(w, [person(0), { ...person(1), first_name: "", national_id: "" }]));
    expect(report.rows.map((r) => r.account_status)).toEqual(["created", "failed: INVALID_STAFF_ROW"]);
    expect(report.rows[1].row_notes).toContain("first_name is required");
    expect(report.rows[1].row_notes).toContain("national_id is required");
  });
});
