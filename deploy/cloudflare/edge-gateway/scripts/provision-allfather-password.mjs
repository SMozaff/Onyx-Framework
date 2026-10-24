#!/usr/bin/env node
/**
 * Owner-controlled ONYX All-Father password provisioning.
 *
 * GitHub Actions credentials do not provision D1. This command hashes the
 * supplied password with the same Argon2id parameters as the Worker and writes
 * only the hash to the existing canonical principal. Test mode is local-D1
 * only. Production mode requires two explicit confirmations and is never
 * invoked by CI or the deploy workflow.
 */
import { argon2id, argon2Verify } from "hash-wasm";
import { randomBytes, randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const workerRoot = path.resolve(scriptDir, "..");
const CANONICAL_ID = "__allfather__";
const CANONICAL_USERNAME = "allfather";
const DATABASE_NAME = "onyx-free-db";

function wranglerEnvironment() {
  const env = { ...process.env };
  delete env.ONYX_ALLFATHER_USERNAME;
  delete env.ONYX_ALLFATHER_PASSWORD;
  return env;
}

export function isCanonicalUsername(value) {
  return typeof value === "string" && value.trim().toLowerCase() === CANONICAL_USERNAME;
}

export function validateCanonicalPrincipal(row) {
  if (!row) return false;
  return row.id === CANONICAL_ID &&
    isCanonicalUsername(row.username) &&
    row.organization_id === CANONICAL_ID &&
    String(row.role) === "ALL_FATHER" &&
    Number(row.is_admin) === 1 &&
    Number(row.is_active) === 1 &&
    Number(row.organization_is_active) === 1 &&
    Number(row.reserved_username_count) === 1;
}

function quote(value) {
  if (typeof value !== "string" || value.includes("\u0000")) throw new Error("invalid SQL literal");
  return "'" + value.replace(/'/g, "''") + "'";
}

export function buildProvisionSql(passwordHash, environment, correlationId = randomUUID()) {
  if (typeof passwordHash !== "string" || !passwordHash.startsWith("$argon2id$")) {
    throw new Error("an Argon2id PHC hash is required");
  }
  if (!["test", "production"].includes(environment)) throw new Error("unsupported environment");
  const details = JSON.stringify({ method: "argon2id", source: "owner-controlled-password-provisioning", environment });
  return `UPDATE users
SET password_hash=${quote(passwordHash)}, updated_at=unixepoch()
WHERE id='__allfather__' AND username='allfather' COLLATE NOCASE
  AND organization_id='__allfather__' AND role='ALL_FATHER'
  AND is_admin=1 AND is_active=1
  AND EXISTS (SELECT 1 FROM organizations o WHERE o.id=users.organization_id AND o.is_active=1);
INSERT INTO audit_log(organization_id,user_id,action,correlation_id,details,created_at)
SELECT organization_id,'__allfather__','allfather.password_provision',${quote(correlationId)},${quote(details)},unixepoch()
FROM users WHERE id='__allfather__' AND password_hash=${quote(passwordHash)};
`;
}

function findD1Rows(value, expectedColumns) {
  if (Array.isArray(value)) {
    if (value.length > 0 && value.every((row) =>
      row !== null && typeof row === "object" && !Array.isArray(row) &&
      expectedColumns.every((column) => Object.hasOwn(row, column))
    )) {
      return value;
    }
    for (const item of value) {
      const rows = findD1Rows(item, expectedColumns);
      if (rows !== null) return rows;
    }
    return null;
  }
  if (value && typeof value === "object") {
    // Wrangler versions have returned both result/results wrappers. Traverse
    // all fields, but only accept an array whose row objects contain the
    // columns required by the SQL statement. This avoids mistaking statement
    // metadata for a successful query row.
    for (const key of ["results", "result"]) {
      if (Object.hasOwn(value, key)) {
        const rows = findD1Rows(value[key], expectedColumns);
        if (rows !== null) return rows;
      }
    }
    for (const [key, child] of Object.entries(value)) {
      if (key !== "results" && key !== "result") {
        const rows = findD1Rows(child, expectedColumns);
        if (rows !== null) return rows;
      }
    }
  }
  return null;
}

export function parseD1Rows(output, expectedColumns = null) {
  const start = output.search(/^\s*[\[{]/m);
  if (start < 0) throw new Error("Wrangler output did not contain JSON");
  const parsed = JSON.parse(output.slice(start));
  // Mutating statements have no result rows to validate; successful process
  // exit plus valid JSON is sufficient. Read queries specify their projection.
  if (expectedColumns === null) return [];
  if (!Array.isArray(expectedColumns) || expectedColumns.length === 0) {
    throw new Error("expected D1 result columns must be specified");
  }
  const rows = findD1Rows(parsed, expectedColumns);
  if (rows === null) throw new Error("Wrangler JSON did not contain the expected D1 row shape");
  return rows;
}

function classifyD1Failure(error) {
  // Wrangler stderr can contain the SQL file contents. Only expose a coarse,
  // allowlisted category; never forward raw stdout/stderr or SQL to CI logs.
  const diagnostic = String(error?.stderr ?? error?.message ?? "");
  if (/unauthorized|invalid api token|authentication error|\b401\b/i.test(diagnostic)) {
    return "Cloudflare authentication was rejected";
  }
  if (/forbidden|\b403\b|permission denied|not authorized/i.test(diagnostic)) {
    return "Cloudflare credentials lack permission for this D1 operation";
  }
  if (/unknown argument|unknown option|unrecognized option|invalid config|configuration.*(error|invalid)|no d1 database binding/i.test(diagnostic)) {
    return "Wrangler rejected the D1 command or generated configuration";
  }
  if (/no such table|no such column|sqlite_error|syntax error/i.test(diagnostic)) {
    return "D1 schema or SQL operation failed";
  }
  if (/database.*not found|not found.*database|unknown database/i.test(diagnostic)) {
    return "configured production D1 database could not be resolved";
  }
  if (/timed out|timeout|econn|network|fetch failed/i.test(diagnostic)) {
    return "Cloudflare D1 request failed due to a network or timeout error";
  }
  return "Wrangler D1 command failed; detailed output suppressed";
}

// Production D1 access goes through the Cloudflare REST query endpoint. In CI,
// `wrangler d1 execute --remote --json` returns an execution summary instead of
// the statement's rows, so reads cannot be verified from its output. The REST
// endpoint returns each statement's results as JSON directly.
let resolvedProductionDatabaseId = null;

function splitStatements(sql) {
  return sql.split(/;\s*\n/).map((s) => s.trim()).filter(Boolean).map((s) => (s.endsWith(";") ? s : s + ";"));
}

async function runD1Rest(sql, { stage, expectedColumns }) {
  const accountId = process.env.CLOUDFLARE_ACCOUNT_ID ?? "";
  const token = process.env.CLOUDFLARE_API_TOKEN ?? "";
  if (!/^[0-9a-f]{32}$/i.test(accountId) || !token || !resolvedProductionDatabaseId) {
    throw new Error(`D1 ${stage} failed: Cloudflare account, token, or database is not configured`);
  }
  const url = `https://api.cloudflare.com/client/v4/accounts/${accountId}/d1/database/${resolvedProductionDatabaseId}/query`;
  let rows = [];
  for (const statement of splitStatements(sql)) {
    let response;
    try {
      response = await fetch(url, {
        method: "POST",
        headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
        body: JSON.stringify({ sql: statement }),
        signal: AbortSignal.timeout(30_000),
      });
    } catch {
      throw new Error(`D1 ${stage} failed: Cloudflare D1 request failed due to a network or timeout error`);
    }
    if (response.status === 401) throw new Error(`D1 ${stage} failed: Cloudflare authentication was rejected`);
    if (response.status === 403) throw new Error(`D1 ${stage} failed: Cloudflare credentials lack permission for this D1 operation`);
    let body;
    try { body = await response.json(); } catch { throw new Error(`D1 ${stage} failed: response was not JSON`); }
    if (!response.ok || body?.success !== true || !Array.isArray(body?.result)) {
      throw new Error(`D1 ${stage} failed: D1 reported an unsuccessful statement`);
    }
    const result = body.result[0] ?? {};
    if (result.success !== true) throw new Error(`D1 ${stage} failed: D1 reported an unsuccessful statement`);
    if (expectedColumns !== undefined && expectedColumns !== null) {
      const statementRows = Array.isArray(result.results) ? result.results : [];
      rows = statementRows.filter((row) => row && typeof row === "object" && expectedColumns.every((c) => Object.hasOwn(row, c)));
      if (statementRows.length !== rows.length) throw new Error(`D1 ${stage} failed: result rows did not match the expected columns`);
    }
  }
  return rows;
}

function runD1(sql, { remote, config, stage, expectedColumns }) {
  if (remote) return runD1Rest(sql, { stage, expectedColumns });
  const dir = mkdtempSync(path.join(tmpdir(), "onyx-allfather-provision-"));
  const file = path.join(dir, "operation.sql");
  try {
    writeFileSync(file, sql, { mode: 0o600 });
    const args = [
      "wrangler", "d1", "execute", DATABASE_NAME,
      remote ? "--remote" : "--local",
      "--config", config,
      "--file", file,
      "--json",
    ];
    const output = execFileSync("npx", args, {
      cwd: workerRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      env: wranglerEnvironment(),
    });
    return parseD1Rows(output, expectedColumns);
  } catch (error) {
    // Wrangler output may echo SQL; only log a stage and an allowlisted category.
    throw new Error(`D1 ${stage} failed: ${classifyD1Failure(error)}`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

function parseWranglerDatabaseList(output) {
  const parsed = JSON.parse(output);
  if (Array.isArray(parsed)) return parsed;
  if (Array.isArray(parsed?.result)) return parsed.result;
  if (Array.isArray(parsed?.databases)) return parsed.databases;
  if (Array.isArray(parsed?.result?.databases)) return parsed.result.databases;
  return [];
}

function createProductionConfig() {
  const output = execFileSync("npx", ["wrangler", "d1", "list", "--json"], {
    cwd: workerRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: wranglerEnvironment(),
  });
  const matches = parseWranglerDatabaseList(output).filter((database) => database?.name === DATABASE_NAME);
  if (matches.length !== 1) throw new Error("expected exactly one configured production D1 database named onyx-free-db");
  const databaseId = matches[0].uuid ?? matches[0].database_id;
  resolvedProductionDatabaseId = typeof databaseId === "string" ? databaseId : null;
  if (typeof databaseId !== "string" || !/^[0-9a-f-]{36}$/i.test(databaseId)) {
    throw new Error("production D1 database ID is missing or malformed");
  }
  const base = readFileSync(path.join(workerRoot, "wrangler.toml"), "utf8");
  const configPath = path.join(workerRoot, `.wrangler-allfather-provision-${process.pid}.toml`);
  const config = base + `
\n[[d1_databases]]
binding = "DB"
database_name = "onyx-free-db"
database_id = "${databaseId}"
migrations_dir = "migrations"
`;
  writeFileSync(configPath, config, { mode: 0o600, flag: "wx" });
  return configPath;
}

async function main(args = process.argv.slice(2)) {
  const option = (name) => {
    const index = args.indexOf(name);
    return index < 0 ? null : args[index + 1] ?? null;
  };
  const environment = option("--environment");
  if (!["test", "production"].includes(environment)) {
    throw new Error("pass --environment test or --environment production");
  }

  const username = process.env.ONYX_ALLFATHER_USERNAME ?? "";
  const password = process.env.ONYX_ALLFATHER_PASSWORD ?? "";
  if (!isCanonicalUsername(username) || !password) {
    throw new Error("ONYX_ALLFATHER_USERNAME must name the reserved account and ONYX_ALLFATHER_PASSWORD must be set");
  }

  let remote = false;
  let config = "wrangler.dev.toml";
  let generatedProductionConfig = null;
  if (environment === "test") {
    if (option("--config") && option("--config") !== "wrangler.dev.toml") {
      throw new Error("test provisioning is restricted to wrangler.dev.toml");
    }
    if (args.includes("--remote")) throw new Error("test provisioning cannot use remote D1");
  } else {
    if (option("--approve-production") !== CANONICAL_ID ||
        option("--expect-database") !== DATABASE_NAME) {
      throw new Error("production requires --approve-production __allfather__ --expect-database onyx-free-db");
    }
    remote = true;
    generatedProductionConfig = createProductionConfig();
    config = path.basename(generatedProductionConfig);
  }

  try {
    const principalSql = `SELECT u.id,u.username,u.organization_id,u.is_active,u.is_admin,u.role,
      o.is_active AS organization_is_active,
      (SELECT COUNT(*) FROM users WHERE LOWER(username)='allfather') AS reserved_username_count
      FROM users u LEFT JOIN organizations o ON o.id=u.organization_id
      WHERE u.id='__allfather__';`;
    const rows = await runD1(principalSql, { remote, config, stage: "canonical-principal check", expectedColumns: ["id", "username", "organization_id", "is_active", "is_admin", "role", "organization_is_active", "reserved_username_count"] });
    if (rows.length !== 1 || !validateCanonicalPrincipal(rows[0])) {
      throw new Error("canonical All-Father principal is missing, ambiguous, inactive, or incorrectly authorized");
    }

    const hash = String(await argon2id({
      password,
      salt: randomBytes(16),
      parallelism: 1,
      iterations: 3,
      memorySize: 65536,
      hashLength: 32,
      outputType: "encoded",
    }));
    await runD1(buildProvisionSql(hash, environment), { remote, config, stage: "password update and audit event" });

    const verified = await runD1(
      "SELECT password_hash FROM users WHERE id='__allfather__' AND username='allfather' AND role='ALL_FATHER' AND is_admin=1 AND is_active=1;",
      { remote, config, stage: "stored-hash verification", expectedColumns: ["password_hash"] },
    );
    if (verified.length !== 1 || !(await argon2Verify({ password, hash: String(verified[0].password_hash ?? "") }))) {
      throw new Error("stored All-Father credential hash could not be verified");
    }
    console.log(`All-Father password provisioned and verified for the canonical principal in ${environment} D1.`);
  } finally {
    if (generatedProductionConfig && existsSync(generatedProductionConfig)) {
      rmSync(generatedProductionConfig, { force: true });
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(`All-Father password provisioning failed: ${error instanceof Error ? error.message : "unknown error"}`);
    process.exitCode = 1;
  });
}
