import { describe, expect, it } from "vitest";
import { buildProvisionSql, isValidUsername, parseD1Rows, validateCanonicalPrincipal } from "../scripts/provision-allfather-password.mjs";

describe("ONYX All-Father password provisioning contract", () => {
  const canonical = {
    id: "__allfather__", username: "allfather", organization_id: "__allfather__",
    is_active: 1, is_admin: 1, role: "ALL_FATHER", organization_is_active: 1,
    conflicting_username_count: 0, conflicting_email_count: 0,
  };

  it("requires the unique canonical active principal and active system organization", () => {
    expect(validateCanonicalPrincipal(canonical)).toBe(true);
    for (const changed of [
      { ...canonical, id: "attacker" },
      { ...canonical, username: "" },
      { ...canonical, role: "ORGANIZATION_ADMIN" },
      { ...canonical, is_admin: 0 },
      { ...canonical, is_active: 0 },
      { ...canonical, organization_is_active: 0 },
      { ...canonical, conflicting_username_count: 1 },
      { ...canonical, conflicting_email_count: 1 },
    ]) expect(validateCanonicalPrincipal(changed)).toBe(false);
  });

  it("extracts D1 rows from Wrangler result wrappers and ignores metadata", () => {
    const principal = {
      id: "__allfather__", username: "allfather", organization_id: "__allfather__",
      is_active: 1, is_admin: 1, role: "ALL_FATHER",
      organization_is_active: 1, conflicting_username_count: 0, conflicting_email_count: 0,
    };
    const wrapped = [
      { success: true, results: [{ success: true, meta: { duration: 1 }, results: [principal] }] },
    ];
    expect(parseD1Rows("Wrangler informational preamble\n" + JSON.stringify(wrapped), [
      "id", "username", "organization_id", "is_active", "is_admin", "role",
      "organization_is_active", "conflicting_username_count", "conflicting_email_count",
    ])).toEqual([principal]);
  });

  it("extracts stored-hash rows and accepts valid mutation JSON without rows", () => {
    expect(parseD1Rows(JSON.stringify([{ success: true, results: [{ password_hash: "$argon2id$test" }] }]), ["password_hash"]))
      .toEqual([{ password_hash: "$argon2id$test" }]);
    expect(parseD1Rows(JSON.stringify([{ success: true, results: [], meta: { changes: 1 } }])))
      .toEqual([]);
  });

  it("validates the username from the secret instead of a fixed name", () => {
    expect(isValidUsername("ops-admin")).toBe(true);
    expect(isValidUsername("allfather")).toBe(true);
    expect(isValidUsername("")).toBe(false);
  });

  it("writes only a PHC Argon2id hash and a non-sensitive audit event", () => {
    const hash = "$argon2id$v=19$m=65536,t=3,p=1$AA$BB";
    const sql = buildProvisionSql(hash, "test", "smoke-correlation", "ops-admin", "owner@example.com");
    expect(sql).toContain("password_hash='$argon2id$");
    expect(sql).toContain("username='ops-admin'");
    expect(sql).toContain("'allfather.password_provision'");
    expect(sql).toContain('"environment":"test"');
    expect(sql).not.toContain("ONYX_ALLFATHER_PASSWORD");
    expect(sql).not.toContain("plain-text-password");
  });

  it("refuses unsupported environments and non-Argon2id hashes", () => {
    expect(() => buildProvisionSql("not-a-hash", "test")).toThrow();
    expect(() => buildProvisionSql("$argon2id$hash", "development")).toThrow();
  });
});
