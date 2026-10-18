import { describe, expect, it, vi } from "vitest";
import { isOnyxLoginSession, loginWithOnyxCredentials } from "../src/auth/credentials";

const session = {
  access_token: "access-token",
  refresh_token: "refresh-token",
  user: {
    id: "__allfather__", username: "allfather", organization_id: "__allfather__",
    role: "ALL_FATHER", is_admin: true, class: null,
  },
};

describe("Admin Shell ONYX credential login contract", () => {
  it("posts username/password to the ONYX login endpoint and consumes the token pair", async () => {
    const post = vi.fn(async () => ({ data: session }));
    const result = await loginWithOnyxCredentials(post, "allfather", "operator-secret");
    expect(post).toHaveBeenCalledExactlyOnceWith("/api/auth/login", {
      username: "allfather", password: "operator-secret", client_type: "admin",
    });
    expect(result).toEqual(session);
    expect(isOnyxLoginSession(result)).toBe(true);
  });

  it("propagates authentication failures without inventing a session", async () => {
    const post = vi.fn(async () => { throw Object.assign(new Error("unauthorized"), { response: { status: 401 } }); });
    await expect(loginWithOnyxCredentials(post, "allfather", "wrong")).rejects.toMatchObject({
      response: { status: 401 },
    });
  });

  it("rejects malformed or incomplete login responses before storing a session", async () => {
    const post = vi.fn(async () => ({ data: { ...session, user: { ...session.user, role: "ADMIN" } } }));
    await expect(loginWithOnyxCredentials(post, "allfather", "secret")).rejects.toThrow("INVALID_AUTH_RESPONSE");
    expect(isOnyxLoginSession({ ...session, access_token: "" })).toBe(false);
  });
});
