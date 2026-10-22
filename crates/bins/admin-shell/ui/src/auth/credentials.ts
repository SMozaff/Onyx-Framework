import type { AuthUser } from "../utils/auth";
import { createLoginRequestId } from "./loginDiagnostics";

export interface OnyxLoginSession {
  access_token: string;
  refresh_token: string;
  user: AuthUser;
}

export interface OnyxLoginPostOptions {
  headers: { "X-ONYX-Request-ID": string };
}

export type OnyxLoginPost = (
  url: string,
  body: { username: string; password: string; client_type: "admin" },
  options: OnyxLoginPostOptions,
) => Promise<{ data: unknown }>;

function isAuthUser(value: unknown): value is AuthUser {
  if (!value || typeof value !== "object") return false;
  const user = value as Record<string, unknown>;
  return typeof user.id === "string" &&
    typeof user.username === "string" &&
    typeof user.organization_id === "string" &&
    typeof user.is_admin === "boolean" &&
    (user.class === null || typeof user.class === "string") &&
    ["ALL_FATHER", "ORGANIZATION_ADMIN", "STAFF"].includes(String(user.role));
}

export function isOnyxLoginSession(value: unknown): value is OnyxLoginSession {
  if (!value || typeof value !== "object") return false;
  const session = value as Record<string, unknown>;
  return typeof session.access_token === "string" && session.access_token.length > 0 &&
    typeof session.refresh_token === "string" && session.refresh_token.length > 0 &&
    isAuthUser(session.user);
}

/** The Admin Shell uses the same ONYX credential endpoint as the Worker contract. */
export async function loginWithOnyxCredentials(
  post: OnyxLoginPost,
  username: string,
  password: string,
  requestId: string = createLoginRequestId(),
): Promise<OnyxLoginSession> {
  const response = await post(
    "/api/auth/login",
    { username, password, client_type: "admin" },
    { headers: { "X-ONYX-Request-ID": requestId } },
  );
  if (!isOnyxLoginSession(response.data)) throw new Error("INVALID_AUTH_RESPONSE");
  return response.data;
}
