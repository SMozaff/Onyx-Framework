/**
 * Server-managed mobile capability policy.
 *
 * Authorization is derived from the persisted user class and the
 * organization-managed allowlist. The caller-controlled client_type claim
 * is deliberately not an input to this decision.
 */

export interface MobileCapabilitySession {
  sub: string;
  organization_id: string;
  role: string;
}

export interface MobileCapabilityEnv {
  DB: D1Database;
}

const MOBILE_ACCESS_CLASSES = [
  "top_level_manager",
  "senior_manager",
  "team_leader",
  "supervisor",
  "staff",
] as const;

type MobileAccessClass = (typeof MOBILE_ACCESS_CLASSES)[number];

function normalizeMobileClasses(value: unknown): MobileAccessClass[] | null {
  if (!Array.isArray(value) || value.length > MOBILE_ACCESS_CLASSES.length) return null;
  if (
    value.some((item) => typeof item !== "string" ||
      !MOBILE_ACCESS_CLASSES.includes(item as MobileAccessClass)) ||
    new Set(value).size !== value.length
  ) return null;

  const selected = new Set(value as MobileAccessClass[]);
  return MOBILE_ACCESS_CLASSES.filter((item) => selected.has(item));
}

async function readAllowedClasses(
  env: MobileCapabilityEnv,
  organizationId: string,
): Promise<MobileAccessClass[] | null> {
  const row = await env.DB.prepare(
    "SELECT allowed_classes,version,updated_at FROM mobile_access_settings WHERE organization_id=? LIMIT 1",
  ).bind(organizationId).first<Record<string, unknown>>();

  // No configured policy means no grant. A missing row is a valid default-deny
  // state; malformed persisted policy data is also denied.
  if (!row) return [];

  let raw: unknown;
  try {
    raw = JSON.parse(String(row.allowed_classes));
  } catch {
    return null;
  }

  const allowed = normalizeMobileClasses(raw);
  const version = Number(row.version);
  const updatedAt = Number(row.updated_at);
  if (
    !allowed ||
    !Number.isSafeInteger(version) ||
    version < 1 ||
    !Number.isSafeInteger(updatedAt) ||
    updatedAt < 0
  ) return null;

  return allowed;
}

/**
 * All-Father and organization administrators can manage policy and are not
 * subject to the staff-class allowlist. Staff access is default-deny and is
 * determined solely by current D1 state scoped to the session organization.
 */
export async function mobileCapabilityGranted(
  env: MobileCapabilityEnv,
  session: MobileCapabilitySession,
): Promise<boolean> {
  if (session.role === "ALL_FATHER" || session.role === "ORGANIZATION_ADMIN") return true;

  const allowedClasses = await readAllowedClasses(env, session.organization_id);
  if (!allowedClasses || allowedClasses.length === 0) return false;

  const actor = await env.DB.prepare(
    "SELECT class FROM users WHERE id=? AND organization_id=? AND is_active=1 LIMIT 1",
  ).bind(session.sub, session.organization_id).first<{ class: string | null }>();

  if (!actor || typeof actor.class !== "string") return false;
  return allowedClasses.includes(actor.class as MobileAccessClass);
}
