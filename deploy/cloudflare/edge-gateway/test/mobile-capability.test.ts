import { describe, expect, it, vi } from "vitest";
import { mobileCapabilityGranted } from "../src/mobile-capability";

const ORG_ID = "11111111-1111-4111-8111-111111111111";
const STAFF_ID = "33333333-3333-4333-8333-333333333333";
const STAFF_CLASS = "staff";

function makeDb(overrides: {
  allowedClasses?: unknown;
  policyRow?: boolean;
  staffClass?: string | null;
  userRow?: boolean;
  policyVersion?: number;
  updatedAt?: number;
} = {}) {
  const allowedClasses = overrides.allowedClasses ?? [STAFF_CLASS];
  const policyRow = overrides.policyRow ?? true;
  const staffClass = overrides.staffClass === undefined ? STAFF_CLASS : overrides.staffClass;
  const userRow = overrides.userRow ?? true;

  return {
    prepare: vi.fn((sql: string) => {
      const statement: any = {
        bind: vi.fn((..._args: unknown[]) => statement),
        first: vi.fn(async () => {
          if (sql.includes("FROM mobile_access_settings")) {
            if (!policyRow) return null;
            return {
              allowed_classes: typeof allowedClasses === "string"
                ? allowedClasses
                : JSON.stringify(allowedClasses),
              version: overrides.policyVersion ?? 1,
              updated_at: overrides.updatedAt ?? Date.now(),
            };
          }
          if (sql.includes("SELECT class FROM users")) {
            return userRow ? { class: staffClass } : null;
          }
          return null;
        }),
      };
      return statement;
    }),
  } as unknown as D1Database;
}

function session(overrides: Record<string, unknown> = {}) {
  return {
    sub: STAFF_ID,
    organization_id: ORG_ID,
    role: "STAFF",
    client_type: "mobile",
    ...overrides,
  };
}

describe("mobileCapabilityGranted", () => {
  it("grants a staff class listed in the current organization policy", async () => {
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: ["staff"] }) },
      session(),
    )).toBe(true);
  });

  it("denies a staff class not listed in the policy", async () => {
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: ["supervisor"] }) },
      session(),
    )).toBe(false);
  });

  it("denies when the policy is empty or missing", async () => {
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: [] }) },
      session(),
    )).toBe(false);
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ policyRow: false }) },
      session(),
    )).toBe(false);
  });

  it("fails closed on malformed or invalid persisted policy", async () => {
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: "not-json" }) },
      session(),
    )).toBe(false);
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: ["staff", "staff"] }) },
      session(),
    )).toBe(false);
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: ["staff"], policyVersion: 0 }) },
      session(),
    )).toBe(false);
  });

  it("does not use client_type as an authorization input", async () => {
    const allowedDb = makeDb({ allowedClasses: ["staff"] });
    for (const clientType of ["mobile", "mobile_observer", "desktop", "web", "admin"]) {
      expect(await mobileCapabilityGranted(
        { DB: allowedDb },
        session({ client_type: clientType }),
      )).toBe(true);
    }

    const deniedDb = makeDb({ allowedClasses: ["supervisor"] });
    for (const clientType of ["mobile", "mobile_observer", "desktop", "web", "admin"]) {
      expect(await mobileCapabilityGranted(
        { DB: deniedDb },
        session({ client_type: clientType }),
      )).toBe(false);
    }
  });

  it("allows organization administrators to manage the policy", async () => {
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: [] }) },
      session({ role: "ORGANIZATION_ADMIN" }),
    )).toBe(true);
  });

  it("allows All-Father to manage the policy", async () => {
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: [] }) },
      session({ role: "ALL_FATHER" }),
    )).toBe(true);
  });

  it("denies when the user is missing, inactive, or has no persisted class", async () => {
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: ["staff"], userRow: false }) },
      session(),
    )).toBe(false);
    expect(await mobileCapabilityGranted(
      { DB: makeDb({ allowedClasses: ["staff"], staffClass: null }) },
      session(),
    )).toBe(false);
  });
});
