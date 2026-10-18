import type { AuthUser } from "@/utils/auth";

export interface AdminNavigationItem {
  to: string;
  label: string;
  end: boolean;
}

const ORGANIZATION_NAVIGATION: AdminNavigationItem = {
  to: "/organizations",
  label: "nav.organizations",
  end: false,
};

const STANDARD_NAVIGATION: AdminNavigationItem[] = [
  { to: "/users", label: "nav.users", end: false },
  { to: "/settings", label: "nav.policySettings", end: false },
  { to: "/profiles", label: "nav.staffProfiles", end: false },
];

/** Client-only UX helper. The Worker is still the authorization boundary. */
export function canViewOrganizations(role: AuthUser["role"] | undefined): boolean {
  return role === "ALL_FATHER";
}

export function navigationForRole(role: AuthUser["role"] | undefined): AdminNavigationItem[] {
  return canViewOrganizations(role) ? [...STANDARD_NAVIGATION, ORGANIZATION_NAVIGATION] : STANDARD_NAVIGATION;
}
