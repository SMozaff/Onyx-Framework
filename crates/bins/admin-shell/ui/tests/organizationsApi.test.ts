import { afterEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({
  get: vi.fn(),
  post: vi.fn(),
  patch: vi.fn(),
  delete: vi.fn(),
}));

vi.mock("../src/api/client", () => ({ apiClient: api }));

import { organizationsApi } from "../src/pages/organizationsApi";

afterEach(() => vi.clearAllMocks());

describe("All-Father organization API wrapper", () => {
  it("lists, creates, and renames organizations using the declared Worker endpoints", async () => {
    api.get.mockResolvedValue({ data: [] });
    api.post.mockResolvedValue({ data: { id: "org-1", name: "North", is_active: true } });
    api.patch.mockResolvedValue({ data: { id: "org-1", name: "North Region", is_active: true } });

    await organizationsApi.listOrganizations();
    await organizationsApi.createOrganization("North");
    await organizationsApi.renameOrganization("org-1", "North Region");

    expect(api.get).toHaveBeenCalledWith("/api/allfather/organizations");
    expect(api.post).toHaveBeenCalledWith("/api/allfather/organizations", { name: "North" });
    expect(api.patch).toHaveBeenCalledWith("/api/allfather/organizations/org-1", { name: "North Region" });
  });

  it("uses organization-specific admin list/create and lifecycle routes", async () => {
    api.get.mockResolvedValue({ data: [] });
    api.post.mockResolvedValue({ data: { success: true } });
    api.delete.mockResolvedValue({ data: { success: true } });

    await organizationsApi.listAdmins("org / 1");
    await organizationsApi.createAdmin({ organization_id: "org-1", username: "north-admin", email: "admin@example.test", password: "correct-horse-8" });
    await organizationsApi.deactivateAdmin("admin-1");
    await organizationsApi.reactivateAdmin("admin-1");
    await organizationsApi.setOrganizationActive("org-1", false);
    await organizationsApi.setOrganizationActive("org-1", true);

    expect(api.get).toHaveBeenCalledWith("/api/allfather/admins?organization_id=org%20%2F%201");
    expect(api.post).toHaveBeenCalledWith("/api/allfather/admins", {
      organization_id: "org-1", username: "north-admin", email: "admin@example.test", password: "correct-horse-8",
    });
    expect(api.delete).toHaveBeenCalledWith("/api/admin/users/admin-1");
    expect(api.post).toHaveBeenCalledWith("/api/allfather/admins/admin-1/reactivate");
    expect(api.post).toHaveBeenCalledWith("/api/allfather/organizations/org-1/deactivate");
    expect(api.post).toHaveBeenCalledWith("/api/allfather/organizations/org-1/activate");
  });
});
