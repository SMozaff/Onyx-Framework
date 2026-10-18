import { apiClient } from "../api/client";

export interface OrganizationRow {
  id: string;
  name: string;
  is_active: boolean;
  created_at: number;
  updated_at: number;
}

export interface OrganizationAdminRow {
  id: string;
  username: string;
  email: string | null;
  organization_id: string;
  is_active: boolean;
  created_at: number;
  updated_at: number;
}

export interface CreateOrganizationAdminInput {
  organization_id: string;
  username: string;
  email: string;
  password: string;
}

const organizationPath = (organizationId: string) => `/api/allfather/organizations/${encodeURIComponent(organizationId)}`;
const adminPath = (adminId: string) => `/api/allfather/admins/${encodeURIComponent(adminId)}`;

/** Uses apiClient exclusively so bearer attachment and refresh interception remain active. */
export const organizationsApi = {
  async listOrganizations(): Promise<OrganizationRow[]> {
    return (await apiClient.get<OrganizationRow[]>("/api/allfather/organizations")).data;
  },
  async createOrganization(name: string): Promise<OrganizationRow> {
    return (await apiClient.post<OrganizationRow>("/api/allfather/organizations", { name })).data;
  },
  async renameOrganization(organizationId: string, name: string): Promise<OrganizationRow> {
    return (await apiClient.patch<OrganizationRow>(organizationPath(organizationId), { name })).data;
  },
  async setOrganizationActive(organizationId: string, active: boolean): Promise<OrganizationRow> {
    const action = active ? "activate" : "deactivate";
    return (await apiClient.post<OrganizationRow>(`${organizationPath(organizationId)}/${action}`)).data;
  },
  async listAdmins(organizationId: string): Promise<OrganizationAdminRow[]> {
    return (await apiClient.get<OrganizationAdminRow[]>(`/api/allfather/admins?organization_id=${encodeURIComponent(organizationId)}`)).data;
  },
  async createAdmin(input: CreateOrganizationAdminInput): Promise<OrganizationAdminRow> {
    return (await apiClient.post<OrganizationAdminRow>("/api/allfather/admins", input)).data;
  },
  async deactivateAdmin(adminId: string): Promise<void> {
    await apiClient.delete(`/api/admin/users/${encodeURIComponent(adminId)}`);
  },
  async reactivateAdmin(adminId: string): Promise<void> {
    await apiClient.post(`${adminPath(adminId)}/reactivate`);
  },
};
