/**
 * Canonical ONYX authentication schema contract for the Cloudflare D1 database.
 *
 * Phase 3 (reports/auth-repair/03-d1-migrations.md): single source of truth
 * for the tables, columns, and indexes the Worker's authentication and
 * authorization paths require. `missingAuthSchema` reports which contract
 * items are absent from a connected database; it performs read-only PRAGMA
 * and no writes.
 *
 * The Worker's `/ready` route uses this check before reporting database
 * readiness. `PRAGMA table_info` and `PRAGMA index_list` are read-only
 * metadata queries and are also covered against SQLite in the migration tests.
 */

export interface SchemaReadClient {
  prepare(sql: string): {
    all(): Promise<
      Array<Record<string, unknown>> | { results?: unknown[] }
    >;
  };
}

/** Required columns per table. Only columns actually used by Worker SQL. */
export const AUTH_SCHEMA_CONTRACT: {
  tables: Record<string, string[]>;
  indexes: string[];
  tableIndexes: Record<string, string[]>;
} = {
  tables: {
    mobile_access_settings: ["organization_id", "allowed_classes", "version", "updated_by", "updated_at", "last_operation_id"],
    users: [
      "id",
      "username",
      "email",
      "password_hash",
      "organization_id",
      "is_admin",
      "is_manager",
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
    user_session_revocations: ["user_id", "revoked_before_ms", "updated_at"],
    replica_ownership: ["replica_id", "user_id", "organization_id", "claimed_at"],
    relay_ticket_redemptions: ["jti", "expires_at", "redeemed_at"],
    audit_log: ["organization_id", "user_id", "action", "correlation_id", "details", "created_at"],
    file_asset_acl: ["content_hash", "organization_id", "user_id", "granted_by", "created_at"],
  },
  indexes: [
    "idx_users_username_lower",
    "idx_users_email_lower",
    "idx_users_supabase_user",
  ],
  tableIndexes: {
    mobile_access_settings: ["idx_mobile_access_settings_updated_at"],
    replica_ownership: ["idx_replica_ownership_org"],
    relay_ticket_redemptions: ["idx_relay_ticket_expiry"],
    file_asset_acl: ["idx_file_asset_acl_user_org"],
  },
};

async function pragmaRows(
  db: SchemaReadClient,
  sql: string,
): Promise<Array<Record<string, unknown>>> {
  const rows = await db.prepare(sql).all();
  // The in-memory SQLite test reader returns a bare array; Cloudflare D1
  // returns a D1Result object with a .results array. Accept both shapes.
  if (Array.isArray(rows)) return rows;
  if (rows && typeof rows === "object" && Array.isArray(rows.results)) {
    return rows.results as Array<Record<string, unknown>>;
  }
  return [];
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
  for (const [table, indexes] of Object.entries(AUTH_SCHEMA_CONTRACT.tableIndexes)) {
    const rows = await pragmaRows(db, `PRAGMA index_list(${table})`);
    const present = new Set(rows.map((row) => String(row.name)));
    for (const index of indexes) {
      if (!present.has(index)) missing.push(`index:${index}`);
    }
  }

  return missing.sort();
}
