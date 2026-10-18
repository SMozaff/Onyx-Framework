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
FROM users WHERE id='__allfather__' AND password_hash=${quote(passwordHash)} AND changes()=1;
`;
}

function parseRows(output) {
  const parsed = JSON.parse(output);
  const candidates = Array.isArray(parsed) ? parsed : [parsed];
  for (const candidate of candidates) {
    if (Array.isArray(candidate?.results)) return candidate.results;
    if (Array.isArray(candidate?.result?.results)) return candidate.result.results;
  }
  if (Array.isArray(parsed?.results)) return parsed.results;
  return [];
}

function runD1(sql, { remote, config }) {
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
      env: process.env,
    });
    return parseRows(output);
  } catch {
    // Wrangler output may echo SQL; never forward it to logs.
    throw new Error("D1 operation failed; SQL output suppressed");
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
    cwd: workerRoot, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], env: process.env,
  });
  const matches = parseWranglerDatabaseList(output).filter((database) => database?.name === DATABASE_NAME);
  if (matches.length !== 1) throw new Error("expected exactly one configured production D1 database named onyx-free-db");
  const databaseId = matches[0].uuid ?? matches[0].database_id;
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

  const username = process.env.ONYX_ALLFATHER_USERNAME ?? "";
  const password = process.env.ONYX_ALLFATHER_PASSWORD ?? "";
  if (!isCanonicalUsername(username) || !password) {
    throw new Error("ONYX_ALLFATHER_USERNAME must name the reserved account and ONYX_ALLFATHER_PASSWORD must be set");
  }

  try {
    const principalSql = `SELECT u.id,u.username,u.organization_id,u.is_active,u.is_admin,u.role,
      o.is_active AS organization_is_active,
      (SELECT COUNT(*) FROM users WHERE LOWER(username)='allfather') AS reserved_username_count
      FROM users u LEFT JOIN organizations o ON o.id=u.organization_id
      WHERE u.id='__allfather__';`;
    const rows = runD1(principalSql, { remote, config });
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
    runD1(buildProvisionSql(hash, environment), { remote, config });

    const verified = runD1(
      "SELECT password_hash FROM users WHERE id='__allfather__' AND username='allfather' AND role='ALL_FATHER' AND is_admin=1 AND is_active=1;",
      { remote, config },
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
