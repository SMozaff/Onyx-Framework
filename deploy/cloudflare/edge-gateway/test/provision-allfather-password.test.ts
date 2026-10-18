import { describe, expect, it } from "vitest";
import {
  buildProvisionSql,
  isCanonicalUsername,
  validateCanonicalPrincipal,
} from "../scripts/provision-allfather-password.mjs";

const principal = {
  id: "__allfather__",
  username: "allfather",
  organization_id: "__allfather__",
  role: "ALL_FATHER",
  is_admin: 1,
  is_active: 1,
  organization_is_active: 1,
  reserved_username_count: 1,
};

describe("ONYX All-Father password provisioning contract", () => {
  it("recognizes only the reserved canonical username, case-insensitively", () => {
    expect(isCanonicalUsername("allfather")).toBe(true);
    expect(isCanonicalUsername("AllFather")).toBe(true);
    expect(isCanonicalUsername("allfather-admin")).toBe(false);
    expect(isCanonicalUsername("")).toBe(false);
  });

  it("requires the exact canonical, unique, active privileged principal", () => {
    expect(validateCanonicalPrincipal(principal)).toBe(true);
    for (const patch of [
      { id: "ordinary-user" },
      { username: "staff" },
      { organization_id: "other-org" },
      { role: "ORGANIZATION_ADMIN" },
      { is_admin: 0 },
      { is_active: 0 },
      { organization_is_active: 0 },
      { reserved_username_count: 2 },
    ]) {
      expect(validateCanonicalPrincipal({ ...principal, ...patch })).toBe(false);
    }
    expect(validateCanonicalPrincipal(null)).toBe(false);
  });

  it("writes only an Argon2id hash to the existing canonical row and audits the operation", () => {
    const hash = "$argon2id$v=19$m=65536,t=3,p=1$YWFhYWFhYWFhYWFhYWFhYQ$YWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWE";
    const sql = buildProvisionSql(hash, "test", "smoke-correlation");
    expect(sql).toContain("SET password_hash=");
    expect(sql).toContain("WHERE id='__allfather__'");
    expect(sql).toContain("role='ALL_FATHER'");
    expect(sql).toContain("is_admin=1 AND is_active=1");
    expect(sql).toContain("o.is_active=1");
    expect(sql).toContain("allfather.password_provision");
    expect(sql).not.toContain("plain-text-password");
    expect(() => buildProvisionSql("plain-text-password", "test")).toThrow();
    expect(() => buildProvisionSql(hash, "remote")).toThrow();
  });
});
