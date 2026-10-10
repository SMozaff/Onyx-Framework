/**
 * Canonical ONYX authentication schema contract for the Cloudflare D1 database.
 *
 * Phase 3 (reports/auth-repair/03-d1-migrations.md): single source of truth
 * for the tables, columns, and indexes the Worker's authentication and
 * authorization paths require. `missingAuthSchema` reports which contract
 * items are absent from a connected database; it performs read-only PRAGMA
 * and no writes.
 *
 * Phase 6 (readiness/observability) should wire this into `/ready` (or an
 * authenticated diagnostics route) instead of inventing a second schema
 * version source. Note: `PRAGMA table_info` / `PRAGMA index_list` are the
 * read-only PRAGMAs; confirm D1 remote acceptance during Phase 6 deployment
 * verification and fall back to `information_schema`-style checks only if
 * D1 rejects them.
 */

export interface SchemaReadClient {
  prepare(sql: string): {
    all(): Promise<Array<Record<string, unknown>>>;
  };
}

/** Required columns per table. Only columns actually used by Worker SQL. */
export const AUTH_SCHEMA_CONTRACT: {
  tables: Record<string, string[]>;
  indexes: string[];
} = {
  tables: {
    users: [
      "id",
      "username",
      "email",
      "password_hash",
      "organization_id",
      "is_admin",
      "is_active",
      "class",
      "role",
      "parent_user_id",
      "supabase_user_id",
      "created_at",
      "updated_at",
    ],
    organizations: ["id", "name", "is_active", "created_at", "updated_at"],
    token_revocations: ["token_hash", "revoked_at"],
    audit_log: ["organization_id", "user_id", "action", "correlation_id", "details", "created_at"],
  },
  indexes: [
    "idx_users_username_lower",
    "idx_users_email_lower",
    "idx_users_supabase_user",
  ],
};

async function pragmaRows(
  db: SchemaReadClient,
  sql: string,
): Promise<Array<Record<string, unknown>>> {
  const rows = await db.prepare(sql).all();
  return Array.isArray(rows) ? rows : [];
}

/**
 * Returns a sorted list of missing contract items, e.g.
 * `users.supabase_user_id`, `table:organizations`, `index:idx_users_supabase_user`.
 * An empty array means the connected database satisfies the auth schema
 * contract. Read-only; safe to run against production with owner approval.
 */
export async function missingAuthSchema(db: SchemaReadClient): Promise<string[]> {
  const missing: string[] = [];

  for (const [table, columns] of Object.entries(AUTH_SCHEMA_CONTRACT.tables)) {
    const info = await pragmaRows(db, `PRAGMA table_info(${table})`);
    if (info.length === 0) {
      missing.push(`table:${table}`);
      continue;
    }
    const present = new Set(info.map((row) => String(row.name)));
    for (const column of columns) {
      if (!present.has(column)) missing.push(`${table}.${column}`);
    }
  }

  const userIndexes = await pragmaRows(db, "PRAGMA index_list(users)");
  const indexNames = new Set(userIndexes.map((row) => String(row.name)));
  for (const index of AUTH_SCHEMA_CONTRACT.indexes) {
    if (!indexNames.has(index)) missing.push(`index:${index}`);
  }

  return missing.sort();
}
