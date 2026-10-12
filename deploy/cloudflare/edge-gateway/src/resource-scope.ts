/**
 * SQL predicates for resource ownership stored in aggregate JSON.
 *
 * The tenant predicate must be applied before ownership. Every non-null
 * owner/assignee/recipient field must match the current Staff principal, and
 * at least one recognized ownership field must be present. Malformed JSON or
 * ownerless records fail closed.
 */
const JSON_OWNER_PATHS = [
  "$.owner_id",
  "$.owner_user_id",
  "$.assignee_id",
  "$.assigned_to",
  "$.user_id",
  "$.recipient_user_id",
  "$.owner.user_id",
] as const;

function jsonExpr(path: string): string {
  return `(CASE WHEN json_valid(state) THEN json_extract(state,'${path}') END)`;
}

export function staffOwnedPredicateSql(): string {
  const expressions = JSON_OWNER_PATHS.map(jsonExpr);
  const anyOwnerMatches = expressions.map((expr) => `${expr} = ?`).join(" OR ");
  const everyOwnerMatchesOrMissing = expressions
    .map((expr) => `(${expr} IS NULL OR ${expr} = ?)`)
    .join(" AND ");
  return `((${anyOwnerMatches}) AND (${everyOwnerMatchesOrMissing}))`;
}

export function staffOwnedBindings(userId: string): string[] {
  return Array(JSON_OWNER_PATHS.length * 2).fill(userId);
}

/**
 * Convert UUID/ObjectId references to the Worker D1 key format. Native Rust
 * ObjectId values can be serialized as a 16-byte array; malformed shapes are
 * not guessed and return null so callers can deny the request.
 */
export function resourceIdFromReference(value: unknown): string | null {
  if (typeof value === "string" && value.trim()) return value.trim();
  if (!Array.isArray(value) || value.length !== 16 ||
      !value.every((part) => Number.isInteger(part) && part >= 0 && part <= 255)) return null;
  const hex = value.map((part: number) => part.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0,8)}-${hex.slice(8,12)}-${hex.slice(12,16)}-${hex.slice(16,20)}-${hex.slice(20)}`;
}
