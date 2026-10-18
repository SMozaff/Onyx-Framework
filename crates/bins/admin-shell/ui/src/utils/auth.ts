const ACCESS_TOKEN_KEY = 'onyx_admin_access_token';
const REFRESH_TOKEN_KEY = 'onyx_admin_refresh_token';
const USER_KEY = 'onyx_admin_user';

export interface AuthUser {
  id: string;
  username: string;
  organization_id: string;
  /** Role returned by the Worker after its authoritative D1 session check. */
  role: "ALL_FATHER" | "ORGANIZATION_ADMIN" | "STAFF";
  /// Added for the Admin platform's client-side "you're not an admin"
  /// check — see `LoginUser::is_admin`'s doc comment in api-server for
  /// the full rationale (this is additive on the shared endpoint;
  /// web-ui/desktop-shell simply don't read it).
  is_admin: boolean;
  class: string | null;
}

export function storeSession(accessToken: string, refreshToken: string, user: AuthUser): void {
  sessionStorage.setItem(ACCESS_TOKEN_KEY, accessToken);
  sessionStorage.setItem(REFRESH_TOKEN_KEY, refreshToken);
  sessionStorage.setItem(USER_KEY, JSON.stringify(user));
}

export function clearSession(): void {
  sessionStorage.removeItem(ACCESS_TOKEN_KEY);
  sessionStorage.removeItem(REFRESH_TOKEN_KEY);
  sessionStorage.removeItem(USER_KEY);
}

export function getAccessToken(): string | null {
  return sessionStorage.getItem(ACCESS_TOKEN_KEY);
}

export function getRefreshToken(): string | null {
  return sessionStorage.getItem(REFRESH_TOKEN_KEY);
}

export function getStoredUser(): AuthUser | null {
  const raw = sessionStorage.getItem(USER_KEY);
  if (!raw) return null;
  try {
    const user = JSON.parse(raw) as Partial<AuthUser>;
    if (!user || typeof user.id !== "string" || typeof user.username !== "string" ||
      typeof user.organization_id !== "string" || typeof user.is_admin !== "boolean" ||
      !["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"].includes(String(user.role))) {
      clearSession();
      return null;
    }
    return user as AuthUser;
  } catch {
    clearSession();
    return null;
  }
}
