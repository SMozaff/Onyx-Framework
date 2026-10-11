#!/usr/bin/env node
/**
 * ONYX All-Father identity provisioning.
 *
 * Phase 4 — reports/auth-repair/04-allfather-provisioning.md
 *
 * The All-Father ONYX principal (`users.id = "__allfather__"`, username
 * `allfather`, role `ALL_FATHER`) is created INERT by the D1 migrations with
 * `users.supabase_user_id = NULL`. Since Phase 2 (AUTH-01) the Worker denies
 * Supabase login for every subject that is not already mapped, so this tool is
 * the only supported way to bind the verified Google/Supabase subject to the
 * one canonical principal. It never creates a principal and never grants a role
 * from an email, username, or Google identity.
 *
 * Guarantees (enforced here and covered by test/provision-allfather.test.ts):
 *  - environment is explicitly selected; production requires an explicit
 *    approval value that restates the subject/principal;
 *  - the canonical principal must already exist and be explicitly ALL_FATHER;
 *  - duplicate/ambiguous ALL_FATHER principals fail closed;
 *  - a subject already bound to a different principal is refused;
 *  - rebinding and activation require explicit flags;
 *  - writes are guarded so a concurrent change cannot be clobbered;
 *  - an audit record is written; no credential material is ever read/written.
 *
 * The DB adapter interface mirrors Cloudflare D1:
 *   db.all(sql, params)  -> Promise<Array<Record<string,unknown>>>
 *   db.run(sql, params)  -> Promise<{ changes?: number|bigint }>
 * The CLI implements it over `wrangler d1 execute --local|--remote --json`.
 */

import { execFile } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const execFileP = promisify(execFile);

export const ALLFATHER_PRINCIPAL_ID = "__allfather__";
export const ALLFATHER_USERNAME = "allfather";
export const ALLFATHER_ROLE = "ALL_FATHER";
export const ENVIRONMENTS = Object.freeze(["local", "preview", "production"]);
export const ACTIONS = Object.freeze(["provision", "revoke"]);

/** Supabase Auth user IDs are UUIDs. Anything else is rejected before any SQL. */
export const SUBJECT_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** Worker-auth columns the provisioning path depends on (schema contract). */
const REQUIRED_USER_COLUMNS = Object.freeze([
  "id",
  "username",
  "role",
  "is_active",
  "is_admin",
  "organization_id",
  "supabase_user_id",
  "password_hash",
]);

export function refusal(code, message, extra = {}) {
  return { status: "refused", code, message, ...extra };
}

/**
 * Validate raw options (CLI flags or direct object). Returns
 * `{ options }` or `{ refusal }` — never throws for user input.
 */
export function normalizeOptions(raw = {}) {
  const action = String(raw.action ?? "provision");
  const environment = String(raw.environment ?? "");
  const subject =
    typeof raw.subject === "string" ? raw.subject.trim().toLowerCase() : "";
  const approval =
    typeof raw.approval === "string" ? raw.approval.trim().toLowerCase() : "";

  if (!ACTIONS.includes(action)) {
    return { refusal: refusal("INVALID_ACTION", `action must be one of ${ACTIONS.join(", ")}`) };
  }
  if (!ENVIRONMENTS.includes(environment)) {
    return {
      refusal: refusal(
        "INVALID_ENVIRONMENT",
        `environment must be one of ${ENVIRONMENTS.join(", ")} (got ${JSON.stringify(raw.environment)})`,
      ),
    };
  }
  if (action === "provision" && !SUBJECT_PATTERN.test(subject)) {
    return {
      refusal: refusal(
        "INVALID_SUBJECT",
        "supabase-user-id must be the Supabase Auth user UUID (do not guess it from an email address)",
      ),
    };
  }
  if (environment === "production") {
    const expected = action === "provision" ? subject : ALLFATHER_PRINCIPAL_ID;
    if (approval !== expected) {
      return {
        refusal: refusal(
          "PRODUCTION_APPROVAL_REQUIRED",
          `production ${action} requires --approve-production equal to ${expected}`,
        ),
      };
    }
  }
  const expectDatabase =
    typeof raw.expectDatabase === "string" ? raw.expectDatabase.trim() : "";
  if (expectDatabase && raw.database && expectDatabase !== String(raw.database)) {
    return {
      refusal: refusal(
        "DATABASE_MISMATCH",
        `--expect-database ${expectDatabase} does not match --database ${raw.database}`,
      ),
    };
  }

  return {
    options: {
      action,
      environment,
      subject: action === "provision" ? subject : "",
      activate: Boolean(raw.activate),
      rebind: Boolean(raw.rebind),
      dryRun: Boolean(raw.dryRun),
      json: Boolean(raw.json),
      database: typeof raw.database === "string" && raw.database ? raw.database : "onyx-free-db",
      config: typeof raw.config === "string" && raw.config ? raw.config : null,
      expectDatabase,
      operator: typeof raw.operator === "string" ? raw.operator.trim() : "",
    },
  };
}

/** Read-only preflight snapshot. Never reads password hashes or secrets. */
export async function readAllFatherState(db, subject) {
  const columns = await db.all("SELECT name FROM pragma_table_info('users')", []);
  const present = new Set(columns.map((row) => String(row.name)));
  const missing = REQUIRED_USER_COLUMNS.filter((column) => !present.has(column));
  if (missing.length > 0) {
    return { schemaMissing: missing, principal: null, privileged: [], subjectBinding: null };
  }

  const principal =
    (
      await db.all(
        `SELECT id, username, role, is_active, is_admin, organization_id, supabase_user_id
           FROM users WHERE id = ? LIMIT 1`,
        [ALLFATHER_PRINCIPAL_ID],
      )
    )[0] ?? null;

  const privileged = await db.all(
    `SELECT id, username, role FROM users
       WHERE LOWER(username) = LOWER(?) OR role = ? ORDER BY id`,
    [ALLFATHER_USERNAME, ALLFATHER_ROLE],
  );

  const subjectBinding =
    subject && SUBJECT_PATTERN.test(subject)
      ? (
          await db.all("SELECT id, username FROM users WHERE supabase_user_id = ? LIMIT 1", [subject])
        )[0] ?? null
      : null;

  return { schemaMissing: [], principal, privileged, subjectBinding };
}

function auditWrite(state, options, action, details) {
  return {
    sql: `INSERT INTO audit_log(organization_id, user_id, action, correlation_id, details, created_at)
          VALUES (?, ?, ?, ?, ?, ?)`,
    params: [
      String(state.principal.organization_id),
      `__owner__:${options.environment}`,
      action,
      details.correlation_id,
      JSON.stringify(details),
      details.created_at,
    ],
  };
}

/**
 * Pure decision function: given the preflight snapshot and options, decide the
 * status and the exact guarded writes. No I/O; deterministic given `deps.now`.
 */
export function planAllFatherAction(options, state, deps = {}) {
  const timestamp = typeof deps.now === "function" ? deps.now() : Date.now();
  const correlationId = deps.correlationId ?? `prov-${timestamp}`;

  if (state.schemaMissing.length > 0) {
    return refusal(
      "AUTH_SCHEMA_MISSING",
      `users is missing required column(s): ${state.schemaMissing.join(", ")}; apply the D1 migrations first`,
    );
  }
  if (!state.principal) {
    return refusal(
      "PRINCIPAL_MISSING",
      `canonical All-Father principal ${ALLFATHER_PRINCIPAL_ID} does not exist; apply the auth migrations (this tool never creates a privileged principal)`,
    );
  }
  const privileged = state.privileged ?? [];
  if (privileged.length !== 1 || String(privileged[0].id) !== ALLFATHER_PRINCIPAL_ID) {
    return refusal(
      "PRINCIPAL_AMBIGUOUS",
      `expected exactly one ALL_FATHER principal ${ALLFATHER_PRINCIPAL_ID}, found: ${privileged
        .map((row) => `${row.id}/role=${row.role ?? "NULL"}`)
        .join(", ") || "none"}; resolve manually — this tool never merges or deletes principals`,
    );
  }
  if (String(state.principal.role ?? "") !== ALLFATHER_ROLE) {
    return refusal(
      "PRINCIPAL_ROLE_NOT_EXPLICIT",
      `principal.role must be the explicit value ${ALLFATHER_ROLE}; role is ${JSON.stringify(state.principal.role ?? null)} (implicit username/email privilege is not accepted)`,
    );
  }
  if (Number(state.principal.is_admin) !== 1) {
    return refusal("PRINCIPAL_NOT_ADMIN", "principal.is_admin must be 1 for the Worker All-Father gate");
  }

  const isActive = Number(state.principal.is_active) === 1;
  if (options.action === "provision" && !isActive && !options.activate) {
    return refusal(
      "PRINCIPAL_INACTIVE",
      "principal is inactive; re-run with --activate only after identity ownership is confirmed",
    );
  }

  const currentSubject =
    state.principal.supabase_user_id == null ? null : String(state.principal.supabase_user_id);

  if (options.action === "revoke") {
    if (currentSubject === null) {
      return {
        status: "already-revoked",
        environment: options.environment,
        subject: null,
        expectedSubject: null,
        writes: [],
        audit: null,
      };
    }
    const details = {
      target: ALLFATHER_PRINCIPAL_ID,
      environment: options.environment,
      action: "revoke",
      previous_subject: currentSubject,
      new_subject: null,
      operator: options.operator || null,
      correlation_id: correlationId,
      created_at: timestamp,
    };
    return {
      status: "revoked",
      environment: options.environment,
      subject: null,
      expectedSubject: null,
      writes: [
        {
          sql: `UPDATE users SET supabase_user_id = NULL, updated_at = ?
                  WHERE id = ? AND supabase_user_id = ?`,
          params: [timestamp, ALLFATHER_PRINCIPAL_ID, currentSubject],
        },
      ],
      audit: auditWrite(state, options, "allfather.revoke", details),
    };
  }

  // action === "provision"
  if (state.subjectBinding && String(state.subjectBinding.id) !== ALLFATHER_PRINCIPAL_ID) {
    return refusal(
      "SUBJECT_BOUND_TO_OTHER_PRINCIPAL",
      `Supabase subject is already mapped to ONYX principal ${state.subjectBinding.id}; refusing to double-bind`,
    );
  }
  if (currentSubject !== null && currentSubject !== options.subject && !options.rebind) {
    return refusal(
      "REBIND_REQUIRES_APPROVAL",
      "principal already has a different Supabase subject; re-run with --rebind only for an approved identity replacement",
    );
  }

  const needsActivate = !isActive;
  const needsMapping = currentSubject !== options.subject;
  if (!needsActivate && !needsMapping) {
    return {
      status: "already-provisioned",
      environment: options.environment,
      subject: options.subject,
      expectedSubject: options.subject,
      writes: [],
      audit: null,
    };
  }

  const writes = [];
  const details = {
    target: ALLFATHER_PRINCIPAL_ID,
    environment: options.environment,
    action: "provision",
    previous_subject: currentSubject,
    new_subject: options.subject,
    activated: needsActivate,
    rebind: currentSubject !== null && currentSubject !== options.subject,
    operator: options.operator || null,
    correlation_id: correlationId,
    created_at: timestamp,
  };
  if (needsActivate) {
    writes.push({
      sql: "UPDATE users SET is_active = 1, updated_at = ? WHERE id = ?",
      params: [timestamp, ALLFATHER_PRINCIPAL_ID],
    });
  }
  if (needsMapping) {
    // Guarded: only writes while the mapping is still NULL or the preflight value.
    writes.push({
      sql: `UPDATE users SET supabase_user_id = ?, updated_at = ?
              WHERE id = ? AND (supabase_user_id IS NULL OR supabase_user_id = ?)`,
      params: [options.subject, timestamp, ALLFATHER_PRINCIPAL_ID, currentSubject],
    });
  }

  return {
    status: "provisioned",
    environment: options.environment,
    subject: options.subject,
    expectedSubject: options.subject,
    writes,
    audit: auditWrite(state, options, "allfather.provision", details),
  };
}

/**
 * Orchestrate preflight → plan → guarded apply → return a result.
 * `deps.now` / `deps.correlationId` exist for deterministic tests.
 */
export async function provisionAllFather(db, rawOptions = {}, deps = {}) {
  const normalized = normalizeOptions(rawOptions);
  if (normalized.refusal) return normalized.refusal;
  const { options } = normalized;

  const state = await readAllFatherState(db, options.subject);
  const plan = planAllFatherAction(options, state, deps);
  if (plan.status === "refused") return plan;

  const plannedStatements = [...plan.writes, ...(plan.audit ? [plan.audit] : [])].map((write) =>
    write.sql.replace(/\s+/g, " ").trim(),
  );
  if (plannedStatements.length === 0 || options.dryRun) {
    return {
      ...plan,
      dryRun: options.dryRun,
      plannedWrites: plannedStatements,
      appliedWrites: 0,
    };
  }

  for (const write of plan.writes) await db.run(write.sql, write.params);

  // Postcondition verification: Wrangler's `d1 execute --json` does not report
  // affected-row counts, so the guard is proven by re-reading the row. If a
  // concurrent change won the race, the value differs and we fail closed
  // WITHOUT writing an audit record that would misstate the outcome.
  const after = await db.all(
    "SELECT supabase_user_id, is_active FROM users WHERE id = ? LIMIT 1",
    [ALLFATHER_PRINCIPAL_ID],
  );
  const actualSubject = after[0]?.supabase_user_id ?? null;
  const actualSubjectValue = actualSubject == null ? null : String(actualSubject);
  if (actualSubjectValue !== plan.expectedSubject) {
    return refusal(
      "CONCURRENT_MODIFICATION",
      `principal no longer matches the preflight state (expected supabase_user_id ${JSON.stringify(
        plan.expectedSubject,
      )}, found ${JSON.stringify(actualSubjectValue)}); no audit record written — re-run`,
    );
  }
  if (options.action === "provision" && Number(after[0]?.is_active) !== 1) {
    return refusal("CONCURRENT_MODIFICATION", "principal is not active after the update; re-run");
  }

  if (plan.audit) await db.run(plan.audit.sql, plan.audit.params);
  return {
    ...plan,
    dryRun: false,
    plannedWrites: plannedStatements,
    appliedWrites: plan.writes.length + (plan.audit ? 1 : 0),
  };
}

/* ------------------------------------------------------------------ *
 * CLI: wrangler d1 execute adapter
 * ------------------------------------------------------------------ */

/** Reject anything that is not a null, finite number, or plain string. */
export function sqlLiteral(value) {
  if (value === null || value === undefined) return "NULL";
  if (typeof value === "number") {
    if (!Number.isFinite(value)) throw new Error("refusing to inline a non-finite number");
    return String(value);
  }
  if (typeof value !== "string") {
    throw new Error(`refusing to inline a ${typeof value} value`);
  }
  if (value.includes("\u0000")) throw new Error("refusing to inline a NUL byte");
  return `'${value.replace(/'/g, "''")}'`;
}

/** Render `?` placeholders with validated literals (the CLI has no bind API). */
export function renderSql(sql, params = []) {
  let index = 0;
  return sql.replace(/\?/g, () => {
    if (index >= params.length) throw new Error("missing SQL parameter");
    return sqlLiteral(params[index++]);
  });
}

export function wranglerAdapter({ database, config, remote }) {
  const args = [
    "wrangler",
    "d1",
    "execute",
    database,
    remote ? "--remote" : "--local",
    ...(config ? ["--config", config] : []),
    "--json",
  ];
  async function execute(sql) {
    const { stdout } = await execFileP("npx", [...args, "--command", sql], {
      maxBuffer: 8 * 1024 * 1024,
    });
    const trimmed = stdout.trim();
    const start = trimmed.indexOf("[");
    if (start < 0) throw new Error(`unexpected wrangler output: ${trimmed.slice(0, 200)}`);
    const parsed = JSON.parse(trimmed.slice(start));
    return Array.isArray(parsed) ? parsed[0] ?? {} : parsed;
  }
  return {
    all: async (sql, params) => (await execute(renderSql(sql, params))).results ?? [],
    run: async (sql, params) => {
      const result = await execute(renderSql(sql, params));
      const changes = result.meta?.changes ?? result.changes;
      return { changes: changes == null ? undefined : Number(changes) };
    },
  };
}

export function parseArgs(argv) {
  const flags = {};
  const withValue = new Set([
    "environment",
    "action",
    "supabase-user-id",
    "database",
    "config",
    "expect-database",
    "operator",
    "approve-production",
  ]);
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith("--")) continue;
    const key = token.slice(2);
    if (key === "help") return { help: true };
    if (withValue.has(key)) {
      flags[key] = argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : "";
    } else {
      flags[key] = true;
    }
  }
  return {
    options: {
      action: flags.action,
      environment: flags.environment,
      subject: flags["supabase-user-id"],
      approval: flags["approve-production"],
      database: flags.database,
      config: flags.config,
      expectDatabase: flags["expect-database"],
      operator: flags.operator,
      activate: Boolean(flags.activate),
      rebind: Boolean(flags.rebind),
      dryRun: Boolean(flags["dry-run"]),
      json: Boolean(flags.json),
    },
  };
}

const USAGE = `Usage: node scripts/provision-allfather.mjs --environment <local|preview|production> [options]

Actions:
  --action provision|revoke        (default: provision)

Provision options:
  --supabase-user-id <uuid>        Supabase Auth user UUID (from the owner's Supabase dashboard)
  --activate                       allow flipping an inactive principal to active
  --rebind                         allow replacing an existing subject (approved identity replacement)

Safety:
  --approve-production <value>     required for --environment production; must restate the subject (provision) or __allfather__ (revoke)
  --expect-database <name>         refuse if it does not match --database (identity pre-check)
  --database <name>                D1 database name (default: onyx-free-db)
  --config <path>                  wrangler config to use (wrangler.toml / wrangler.deploy.toml)
  --operator <label>               recorded in the audit details
  --dry-run                        preflight + plan only, no writes
  --json                           machine-readable output

Never stores credentials; never creates a principal; never grants ALL_FATHER from an email/username.`;

async function main() {
  const parsed = parseArgs(process.argv.slice(2));
  if (parsed.help) {
    process.stdout.write(`${USAGE}\n`);
    return 0;
  }
  const raw = parsed.options;
  const normalized = normalizeOptions(raw);
  if (normalized.refusal) {
    process.stderr.write(`${normalized.refusal.code}: ${normalized.refusal.message}\n`);
    return 2;
  }
  const { options } = normalized;
  const remote = options.environment !== "local";
  const config = options.config ?? (options.environment === "production" ? "wrangler.deploy.toml" : null);
  if (config && !existsSync(config)) {
    process.stderr.write(`CONFIG_NOT_FOUND: ${config}\n`);
    return 2;
  }

  try {
    const db = wranglerAdapter({ database: options.database, config, remote });
    const result = await provisionAllFather(db, { ...raw, ...options });
    if (options.json) process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    else {
      const lines = [`status: ${result.dryRun ? `${result.status} (dry-run)` : result.status}`];
      if (result.environment) lines.push(`environment: ${result.environment}`);
      if (result.subject) lines.push(`subject: ${result.subject}`);
      if (result.code) lines.push(`code: ${result.code}`);
      if (result.message) lines.push(`message: ${result.message}`);
      if (result.plannedWrites) lines.push(`planned writes: ${result.plannedWrites.length}`);
      if (result.appliedWrites != null) lines.push(`applied writes: ${result.appliedWrites}`);
      process.stdout.write(`${lines.join("\n")}\n`);
    }
    return result.status === "refused" ? 2 : 0;
  } catch (error) {
    process.stderr.write(`UNEXPECTED: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  main().then((code) => process.exit(code));
}
