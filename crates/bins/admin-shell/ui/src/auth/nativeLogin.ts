import { invoke } from "@tauri-apps/api/core";

/**
 * Sends the admin login through the native Rust command `admin_login`
 * instead of from the WebView. The Rust side owns the request, timeout,
 * and endpoint, so WebView network behavior no longer affects sign-in.
 *
 * Errors are converted to the same Axios-like shape that
 * `classifyLoginFailure` already understands, so the stable login error
 * codes and messages stay unchanged.
 */
export async function nativeAdminLogin(
  body: { username: string; password: string },
  requestId: string,
): Promise<{ data: unknown }> {
  try {
    const data = await invoke<unknown>("admin_login", {
      input: { username: body.username, password: body.password, request_id: requestId },
    });
    return { data };
  } catch (raw) {
    throw toClassifiableError(raw);
  }
}

type NativeLoginError =
  | { kind: "http"; status: number; code: string | null }
  | { kind: "timeout" }
  | { kind: "connect" }
  | { kind: "network" }
  | { kind: "invalid_response" };

function toClassifiableError(raw: unknown): Error & Record<string, unknown> {
  const failure = (raw ?? {}) as Partial<NativeLoginError> & { kind?: string };
  const error = new Error(
    failure.kind === "invalid_response" ? "INVALID_AUTH_RESPONSE" : "LOGIN_NATIVE_FAILED",
  ) as Error & Record<string, unknown>;
  error.isAxiosError = true;

  switch (failure.kind) {
    case "http": {
      const status = typeof failure.status === "number" ? failure.status : undefined;
      const code = typeof failure.code === "string" ? failure.code : undefined;
      error.response = { status, data: code ? { code } : {} };
      break;
    }
    case "timeout":
      error.code = "ECONNABORTED";
      break;
    case "connect":
      error.code = "ECONNREFUSED";
      break;
    case "network":
      error.code = "ERR_NETWORK";
      break;
    default:
      // invalid_response, or anything unrecognized: no transport code, so the
      // classifier does not label it as a network failure.
      break;
  }
  return error;
}
