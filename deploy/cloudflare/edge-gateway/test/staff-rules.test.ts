/**
 * Pure staff rules: ID layout (mirrors id_generator.rs), Asia/Tehran year
 * boundary, row validation (mirrors staff_sheet.rs), one-time passwords.
 */
import { describe, expect, it } from "vitest";
import {
  generateOneTimePassword,
  initialsFrom,
  missingRequiredColumn,
  ONE_TIME_PASSWORD_LENGTH,
  prepareStaffRow,
  staffIdFor,
  tehranGregorianYear,
} from "../src/staff-rules";

describe("staff ID layout (mirrors crates/bins/api-server/src/routes/profiles/id_generator.rs)", () => {
  it("matches the documented example st26sm0035", () => {
    expect(staffIdFor(2026, "Soheil", "Mozaffari", 35)).toBe("st26sm0035");
  });

  it("zero-pads the order and lowercases initials", () => {
    expect(staffIdFor(2026, "ALI", "ahmadi", 1)).toBe("st26aa0001");
    expect(staffIdFor(2026, "Ali", "Ahmadi", 9999)).toBe("st26aa9999");
  });

  it("enforces the order and year bounds", () => {
    expect(() => staffIdFor(2026, "A", "B", 0)).toThrow("ORDER_OUT_OF_RANGE");
    expect(() => staffIdFor(2026, "A", "B", 10000)).toThrow("ORDER_OUT_OF_RANGE");
    expect(() => staffIdFor(1999, "A", "B", 1)).toThrow("YEAR_OUT_OF_RANGE");
    expect(() => staffIdFor(2100, "A", "B", 1)).toThrow("YEAR_OUT_OF_RANGE");
  });

  it("takes the first Latin letter, and refuses names with none", () => {
    expect(initialsFrom("Soheil", "مظفری")).toBeNull();
    expect(initialsFrom("سهیل", "Mozaffari")).toBeNull();
    expect(initialsFrom("", "Mozaffari")).toBeNull();
    expect(initialsFrom("سهیل", "مظفری")).toBeNull();
  });
});

describe("Asia/Tehran year boundary (docs/design/id-sequences.md)", () => {
  it("uses the Tehran calendar year, not UTC", () => {
    // 2026-12-31 20:29 UTC = 23:59 Tehran (still 2026).
    expect(tehranGregorianYear(Date.parse("2026-12-31T20:29:00Z"))).toBe(2026);
    // 2026-12-31 20:31 UTC = 00:01 on 2027-01-01 Tehran.
    expect(tehranGregorianYear(Date.parse("2026-12-31T20:31:00Z"))).toBe(2027);
  });
});

describe("staff row validation (mirrors crates/bins/api-server/src/routes/profiles/staff_sheet.rs)", () => {
  const good = {
    first_name: "Soheil", last_name: "Mozaffari", national_id: "0012345678",
    contact_email: "Person@Example.TEST", team_ids: "tm261030sm0001; tm261030sm0001;;",
  };

  it("accepts a valid row, lowercases the email, and de-duplicates teams", () => {
    const result = prepareStaffRow(good, 2);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.row.contactEmail).toBe("person@example.test");
    expect(result.row.teamIds).toEqual(["tm261030sm0001"]);
    expect(result.row.nationalId).toBe("0012345678");
    expect(result.row.sourceRow).toBe(2);
  });

  it("requires exactly 10 national ID digits, stored as text", () => {
    expect(prepareStaffRow({ ...good, national_id: "123456789" }, 2)).toMatchObject({ ok: false });
    expect(prepareStaffRow({ ...good, national_id: "12345abcde" }, 2)).toMatchObject({ ok: false });
    const numeric = prepareStaffRow({ ...good, national_id: 12345678 }, 2);
    expect(numeric.ok).toBe(false);
    if (!numeric.ok) expect(numeric.messages).toContain("national_id must be stored as text, not a number");
  });

  it("collects every problem in the row, without echoing values", () => {
    const secret = "NOT-A-NAME-" + crypto.randomUUID();
    const result = prepareStaffRow({ first_name: "", last_name: secret, national_id: "", contact_email: "not-an-email" }, 3);
    expect(result.ok).toBe(false);
    if (result.ok) return;
    expect(result.messages).toEqual(expect.arrayContaining([
      "first_name is required",
      "national_id is required",
      "contact_email is not a valid email address",
    ]));
    expect(result.messages.join(" ")).not.toContain(secret);
  });

  it("records a supplied onyx_id as ignored, and a blank contact email for follow-up", () => {
    const result = prepareStaffRow({ ...good, onyx_id: "st00aa0001", contact_email: "" }, 2);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.row.suppliedOnyxId).toBe("st00aa0001");
    expect(result.row.contactEmail).toBeNull();
    expect(result.notes.join(" ")).toContain("supplied onyx_id was ignored");
    expect(result.notes.join(" ")).toContain("placeholder");
  });

  it("finds a missing required column by name", () => {
    expect(missingRequiredColumn(["onyx_id", "first_name", "last_name", "national_id"])).toBeNull();
    expect(missingRequiredColumn(["onyx_id", "first_name", "last_name"])).toBe("national_id");
  });
});

describe("one-time password generator", () => {
  it("produces passwords of the configured length from the unambiguous alphabet", () => {
    for (let i = 0; i < 50; i++) {
      const password = generateOneTimePassword();
      expect(password).toHaveLength(ONE_TIME_PASSWORD_LENGTH);
      expect(ONE_TIME_PASSWORD_LENGTH).toBeGreaterThanOrEqual(12);
      expect(password).toMatch(/^[A-HJ-NP-Za-km-z2-9]+$/);
    }
  });
});
