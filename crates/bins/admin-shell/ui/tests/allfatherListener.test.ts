import { describe, expect, it } from "vitest";
import {
  allfatherListenerReducer,
  canStartAllfatherSignIn,
  type AllfatherListenerStatus,
} from "../src/auth/allfatherListener";

describe("All-Father callback listener readiness", () => {
  it("keeps sign-in disabled after registration failure and exposes a retry state", () => {
    let status: AllfatherListenerStatus = "checking";
    expect(canStartAllfatherSignIn(status)).toBe(false);

    status = allfatherListenerReducer(status, "registration-failed");
    expect(status).toBe("failed");
    expect(canStartAllfatherSignIn(status)).toBe(false);

    status = allfatherListenerReducer(status, "retry");
    expect(status).toBe("checking");
    expect(canStartAllfatherSignIn(status)).toBe(false);
  });

  it("enables sign-in only after a retry successfully registers the listener", () => {
    let status: AllfatherListenerStatus = "failed";
    status = allfatherListenerReducer(status, "retry");
    status = allfatherListenerReducer(status, "registered");

    expect(status).toBe("ready");
    expect(canStartAllfatherSignIn(status)).toBe(true);
  });
});
