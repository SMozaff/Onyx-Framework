import { describe, expect, it } from "vitest";
import { canViewOrganizations, navigationForRole } from "../src/components/Layout/adminNavigation";

describe("All-Father organization navigation", () => {
  it("shows organization controls only for the All-Father role", () => {
    expect(canViewOrganizations("ALL_FATHER")).toBe(true);
    expect(canViewOrganizations("ORGANIZATION_ADMIN")).toBe(false);
    expect(canViewOrganizations("STAFF")).toBe(false);
    expect(canViewOrganizations(undefined)).toBe(false);
  });

  it("keeps the organization route out of ordinary administrator navigation", () => {
    expect(navigationForRole("ORGANIZATION_ADMIN").map((item) => item.to)).not.toContain("/organizations");
    expect(navigationForRole("ALL_FATHER").map((item) => item.to)).toContain("/organizations");
  });
});
