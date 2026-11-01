/**
 * Staff import rules shared by the batch import (.xlsx, CSV, JSON) and single
 * creation. Pure functions, no D1 access.
 *
 * Mirrors the Rust reference:
 * - column and validation rules: crates/bins/api-server/src/routes/profiles/staff_sheet.rs
 * - ID layout and initials:      crates/bins/api-server/src/routes/profiles/id_generator.rs
 * Rules: docs/product/staff-and-team-rules.md. ID allocation: docs/design/id-sequences.md.
 */

export const REQUIRED_STAFF_COLUMNS = ["onyx_id", "first_name", "last_name", "national_id"] as const;

/** Mirrors the placeholder rule in docs/migrations/slint/decision-log.md (Email). */
export const PLACEHOLDER_EMAIL_DOMAIN = "accounts.invalid";

/** Asia/Tehran has been a fixed UTC+03:30 offset with no daylight saving since 2022. */
const TEHRAN_OFFSET_MS = 3.5 * 60 * 60 * 1000;

export interface PreparedStaffRow {
  sourceRow: number;
  firstName: string;
  lastName: string;
  firstNameFa: string;
  lastNameFa: string;
  nationalId: string;
  /** Null when the sheet left contact_email blank; the placeholder is then used. */
  contactEmail: string | null;
  contactPhone: string | null;
  jobTitle: string | null;
  department: string | null;
  classLabel: string | null;
  parentOnyxId: string | null;
  teamIds: string[];
  /** A supplied onyx_id. Always ignored; reported in the row notes. */
  suppliedOnyxId: string | null;
}

export type RawStaffValue = string | number | boolean | null | undefined | string[];
export type RawStaffRow = Record<string, RawStaffValue>;

export type StaffRowResult =
  | { ok: true; row: PreparedStaffRow; notes: string[] }
  | { ok: false; messages: string[] };

/** Gregorian year in Asia/Tehran, which decides the year of every generated ID. */
export function tehranGregorianYear(nowMs: number = Date.now()): number {
  return new Date(nowMs + TEHRAN_OFFSET_MS).getUTCFullYear();
}

/** First ASCII letter of a name, lowercased. Null when the name has none. */
export function firstAsciiLetter(name: string): string | null {
  const match = name.trim().match(/[A-Za-z]/);
  return match ? match[0].toLowerCase() : null;
}

/** Two-letter initials from the English first and last names. Null if either has no Latin letter. */
export function initialsFrom(firstName: string, lastName: string): string | null {
  const first = firstAsciiLetter(firstName);
  const last = firstAsciiLetter(lastName);
  return first && last ? `${first}${last}` : null;
}

/** Reference formatter (same output as Rust `staff_id`). Used by tests; the Worker builds IDs in SQL. */
export function staffIdFor(year: number, firstName: string, lastName: string, order: number): string {
  if (year < 2000 || year > 2099) throw new Error("YEAR_OUT_OF_RANGE");
  if (!Number.isInteger(order) || order < 1 || order > 9999) throw new Error("ORDER_OUT_OF_RANGE");
  const initials = initialsFrom(firstName, lastName);
  if (!initials) throw new Error("NON_LATIN_NAME");
  return `st${String(year - 2000).padStart(2, "0")}${initials}${String(order).padStart(4, "0")}`;
}

const PASSWORD_ALPHABET = "ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789";
export const ONE_TIME_PASSWORD_LENGTH = 16;

/**
 * One-time password for a new account. Uniform over the alphabet (rejection
 * sampling), drawn from the platform CSPRNG. Ambiguous glyphs (0/O, 1/l/I) are
 * excluded so the printed report can be read back.
 */
export function generateOneTimePassword(): string {
  const limit = 256 - (256 % PASSWORD_ALPHABET.length);
  let out = "";
  const buffer = new Uint8Array(1);
  while (out.length < ONE_TIME_PASSWORD_LENGTH) {
    crypto.getRandomValues(buffer);
    if (buffer[0] < limit) out += PASSWORD_ALPHABET[buffer[0] % PASSWORD_ALPHABET.length];
  }
  return out;
}

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
const LIMITS: Record<string, number> = {
  first_name: 100,
  last_name: 100,
  first_name_fa: 100,
  last_name_fa: 100,
  contact_email: 254,
  contact_phone: 32,
  job_title: 200,
  department: 200,
  class_label: 100,
  parent_onyx_id: 32,
};

function textValue(value: RawStaffValue): string {
  if (value === null || value === undefined) return "";
  if (Array.isArray(value)) return value.join(";");
  return String(value).trim();
}

/**
 * Validates one staff row. Every problem is collected (like the Rust parser),
 * so the report can say everything that is wrong with the row at once.
 * Messages name the column but never echo the submitted value.
 */
export function prepareStaffRow(raw: RawStaffRow, sourceRow: number): StaffRowResult {
  const messages: string[] = [];
  const notes: string[] = [];
  const text = (key: string) => textValue(raw[key]);

  const firstName = text("first_name");
  const lastName = text("last_name");
  if (!firstName) messages.push("first_name is required");
  if (!lastName) messages.push("last_name is required");
  if (firstName && !firstAsciiLetter(firstName)) messages.push("first_name must contain a Latin letter");
  if (lastName && !firstAsciiLetter(lastName)) messages.push("last_name must contain a Latin letter");

  const nationalValue = raw.national_id;
  const nationalId = text("national_id");
  if (typeof nationalValue === "number" || typeof nationalValue === "boolean") {
    messages.push("national_id must be stored as text, not a number");
  } else if (!nationalId) {
    messages.push("national_id is required");
  } else if (!/^[0-9]{10}$/.test(nationalId)) {
    messages.push("national_id must be exactly 10 digits");
  }

  const contactEmail = text("contact_email").toLowerCase() || null;
  if (contactEmail && (contactEmail.length > LIMITS.contact_email || !EMAIL_PATTERN.test(contactEmail))) {
    messages.push("contact_email is not a valid email address");
  }

  for (const key of Object.keys(LIMITS)) {
    if (key === "contact_email") continue;
    const value = text(key);
    if (value.length > LIMITS[key]) messages.push(`${key} is longer than ${LIMITS[key]} characters`);
  }

  const teamIds: string[] = [];
  const rawTeams = Array.isArray(raw.team_ids) ? raw.team_ids : text("team_ids").split(";");
  for (const id of rawTeams.map((x) => String(x).trim()).filter(Boolean)) {
    if (id.length > 64) { messages.push("team_ids contains an entry longer than 64 characters"); break; }
    if (!teamIds.includes(id)) teamIds.push(id);
  }
  if (teamIds.length > 50) messages.push("team_ids contains more than 50 teams");

  const suppliedOnyxId = text("onyx_id") || null;
  if (messages.length > 0) return { ok: false, messages };

  if (suppliedOnyxId) notes.push("supplied onyx_id was ignored; a new staff ID was generated");
  if (!contactEmail) notes.push(`contact_email blank; placeholder account email used (follow up)`);

  return {
    ok: true,
    notes,
    row: {
      sourceRow,
      firstName,
      lastName,
      firstNameFa: text("first_name_fa"),
      lastNameFa: text("last_name_fa"),
      nationalId,
      contactEmail,
      contactPhone: text("contact_phone") || null,
      jobTitle: text("job_title") || null,
      department: text("department") || null,
      classLabel: text("class_label") || null,
      parentOnyxId: text("parent_onyx_id").toLowerCase() || null,
      teamIds,
      suppliedOnyxId,
    },
  };
}

/** Column check for tabular files (xlsx, CSV). Returns the first missing column, or null. */
export function missingRequiredColumn(headers: string[]): string | null {
  const present = new Set(headers.map((h) => h.trim().toLowerCase()));
  for (const column of REQUIRED_STAFF_COLUMNS) if (!present.has(column)) return column;
  return null;
}
