import { describe, expect, it } from "vitest";
import {
  buildProvisionSql,
  isValidEmail,
  isValidUsername,
  validateCanonicalPrincipal,
} from "../scripts/provision-allfather-password.mjs";

const principal = {
  id: "__allfather__",
  username: "ops-admin",
  organization_id: "__allfather__",
  role: "ALL_FATHER",
  is_admin: 1,
  is_active: 1,
  organization_is_active: 1,
  conflicting_username_count: 0,
  conflicting_email_count: 0,
};

describe("ONYX All-Father password provisioning contract", () => {
  it("accepts any well-formed username from the secret, and rejects malformed ones", () => {
    expect(isValidUsername("ops-admin")).toBe(true);
    expect(isValidUsername("Owner.Root_1")).toBe(true);
    expect(isValidUsername("ab")).toBe(false);
    expect(isValidUsername("")).toBe(false);
    expect(isValidUsername("has space")).toBe(false);
    expect(isValidUsername("x';DROP TABLE users;--")).toBe(false);
    expect(isValidUsername(undefined)).toBe(false);
  });

  it("validates the optional email from the secret", () => {
    expect(isValidEmail("owner@example.com")).toBe(true);
    expect(isValidEmail("not-an-email")).toBe(false);
    expect(isValidEmail("a'b@example.com")).toBe(true);
    expect(isValidEmail("x@y.z" + "a".repeat(260))).toBe(false);
  });

  it("requires the exact canonical, unique, active privileged principal", () => {
    expect(validateCanonicalPrincipal(principal)).toBe(true);
    for (const patch of [
      { id: "ordinary-user" },
      { username: "" },
      { organization_id: "other-org" },
      { role: "ORGANIZATION_ADMIN" },
      { is_admin: 0 },
      { is_active: 0 },
      { organization_is_active: 0 },
      { conflicting_username_count: 1 },
      { conflicting_email_count: 1 },
    ]) {
      expect(validateCanonicalPrincipal({ ...principal, ...patch })).toBe(false);
    }
    expect(validateCanonicalPrincipal(null)).toBe(false);
  });

  it("writes only an Argon2id hash to the existing canonical row and audits the operation", () => {
    const hash = "$argon2id$v=19$m=65536,t=3,p=1$YWFhYWFhYWFhYWFhYWFhYQ$YWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWFhYWE";
    const sql = buildProvisionSql(hash, "test", "smoke-correlation", "ops-admin", "owner@example.com");
    expect(sql).toContain("SET password_hash=");
    expect(sql).toContain("username='ops-admin'");
    expect(sql).toContain("email='owner@example.com'");
    expect(sql).not.toContain("username='allfather'");
    expect(sql).toContain("WHERE id='__allfather__'");
    expect(sql).toContain("role='ALL_FATHER'");
    expect(sql).toContain("is_admin=1 AND is_active=1");
    expect(sql).toContain("o.is_active=1");
    expect(sql).toContain("allfather.password_provision");
    expect(sql).not.toContain("plain-text-password");
    expect(sql).not.toMatch(/supabase|google|identity_sub/i);
    expect(() => buildProvisionSql("plain-text-password", "test")).toThrow();
    expect(() => buildProvisionSql(hash, "remote", undefined, "ops-admin")).toThrow();
    expect(() => buildProvisionSql(hash, "test", undefined, "bad name")).toThrow();
    expect(() => buildProvisionSql(hash, "test", undefined, "ops-admin", "bad-email")).toThrow();
  });
});
