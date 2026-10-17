/**
 * Regression tests for the All-Father Supabase sign-in (Authorization Code Flow
 * with PKCE) that replaced the old `flow_type=implicit` fragment-token flow.
 *
 * Covered: RFC 7636 verifier/challenge construction, authorize-URL shape,
 * CSRF `state` binding (missing / mismatched / expired / replayed), callback
 * parsing and parameter scrubbing, provider denial, malformed and legacy
 * implicit responses, and the code exchange — including that no credential
 * material is logged and no client secret is used.
 *
 * Every external dependency is injected, so these run without a browser.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  ALLFATHER_PKCE_STORAGE_KEY,
  AllfatherOAuthError,
  beginAllfatherSignIn,
  buildAuthorizeUrl,
  clearTransaction,
  completeAllfatherCallback,
  deriveCodeChallenge,
  exchangeCodeForSession,
  generateCodeVerifier,
  generateState,
  loadTransaction,
  parseCallback,
  resolveAllfatherRedirectTo,
  saveTransaction,
  stripOAuthParams,
  STATE_TTL_MS,
  type StorageLike,
} from "../src/auth/allfatherOAuth";

const SUPABASE_URL = "https://example-project.supabase.co";
const PUBLISHABLE_KEY = "pk_test_dummy_publishable_key";
const ORIGIN = "http://localhost:5174/login";
const REDIRECT_TO = `${ORIGIN}?auth=allfather`;
const CONFIG = { supabaseUrl: SUPABASE_URL, publishableKey: PUBLISHABLE_KEY };

function memoryStorage(): StorageLike & { dump(): Record<string, string> } {
  const map = new Map<string, string>();
  return {
    getItem: (key) => (map.has(key) ? map.get(key)! : null),
    setItem: (key, value) => void map.set(key, value),
    removeItem: (key) => void map.delete(key),
    dump: () => Object.fromEntries(map),
  };
}

let storage: ReturnType<typeof memoryStorage>;
let clock = 1_000_000;
const deps = () => ({ storage, now: () => clock });

beforeEach(() => {
  storage = memoryStorage();
  clock = 1_000_000;
});

afterEach(() => {
  vi.restoreAllMocks();
});

/* ------------------------------------------------------------------ *
 * PKCE primitives
 * ------------------------------------------------------------------ */

describe("PKCE primitives", () => {
  it("generates an RFC 7636 compliant 43-character verifier", () => {
    const verifier = generateCodeVerifier();
    expect(verifier).toMatch(/^[A-Za-z0-9\-._~]{43,128}$/);
    expect(generateCodeVerifier()).not.toBe(verifier);
  });

  it("derives the RFC 7636 Appendix B reference challenge (S256)", async () => {
    // Official RFC 7636 test vector.
    const verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
    await expect(deriveCodeChallenge(verifier)).resolves.toBe("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
  });

  it("generates an unpredictable state nonce distinct from the verifier", async () => {
    const verifier = generateCodeVerifier();
    const state = generateState();
    expect(state).toMatch(/^[A-Za-z0-9\-._~]{22,}$/);
    expect(state).not.toBe(verifier);
    expect(generateState()).not.toBe(state);
    // The challenge must not be reversible to the verifier in the URL.
    expect(await deriveCodeChallenge(verifier)).not.toBe(verifier);
  });
});

/* ------------------------------------------------------------------ *
 * Authorize URL
 * ------------------------------------------------------------------ */

describe("buildAuthorizeUrl", () => {
  it("matches Supabase's PKCE query shape and never requests the implicit flow", async () => {
    const verifier = generateCodeVerifier();
    const challenge = await deriveCodeChallenge(verifier);
    const state = generateState();
    const url = new URL(buildAuthorizeUrl({ ...CONFIG, redirectTo: REDIRECT_TO, codeChallenge: challenge, state }));

    expect(url.origin + url.pathname).toBe(`${SUPABASE_URL}/auth/v1/authorize`);
    expect(url.searchParams.get("provider")).toBe("google");
    expect(url.searchParams.get("code_challenge")).toBe(challenge);
    expect(url.searchParams.get("code_challenge_method")).toBe("s256");
    expect(url.searchParams.get("apikey")).toBe(PUBLISHABLE_KEY);
    // The implicit flow must be gone entirely.
    expect(url.searchParams.get("flow_type")).toBeNull();
    expect(url.toString()).not.toContain("implicit");
    // Supabase owns `state` internally; our nonce travels inside redirect_to.
    expect(url.searchParams.get("state")).toBeNull();

    const redirect = new URL(url.searchParams.get("redirect_to")!);
    expect(redirect.searchParams.get("auth")).toBe("allfather");
    expect(redirect.searchParams.get("state")).toBe(state);
  });
});

describe("beginAllfatherSignIn", () => {
  it("persists the verifier + state for this tab and returns the authorize URL", async () => {
    const { authorizeUrl, state } = await beginAllfatherSignIn({ ...CONFIG, redirectTo: REDIRECT_TO }, deps());

    const stored = loadTransaction(storage);
    expect(stored?.state).toBe(state);
    expect(stored?.verifier).toMatch(/^[A-Za-z0-9\-._~]{43}$/);
    expect(stored?.createdAt).toBe(clock);

    const url = new URL(authorizeUrl);
    expect(url.searchParams.get("code_challenge")).toBe(await deriveCodeChallenge(stored!.verifier));
    // The verifier itself must never be handed to the authorize URL.
    expect(url.toString()).not.toContain(stored!.verifier);
  });

  it("refuses to start when Supabase is not configured", async () => {
    await expect(
      beginAllfatherSignIn({ supabaseUrl: "", publishableKey: "", redirectTo: REDIRECT_TO }, deps()),
    ).rejects.toMatchObject({ code: "MISSING_CONFIG" });
    expect(storage.dump()).toEqual({});
  });
});

/* ------------------------------------------------------------------ *
 * Callback parsing / scrubbing
 * ------------------------------------------------------------------ */

describe("parseCallback", () => {
  it("reads the code and state from the query", () => {
    const params = parseCallback(`${REDIRECT_TO}&state=abc&code=the-code`);
    expect(params).toMatchObject({ code: "the-code", state: "abc", error: null, legacyTokenInFragment: false });
  });

  it("surfaces a provider denial", () => {
    const params = parseCallback(`${REDIRECT_TO}&error=access_denied&error_description=User+denied`);
    expect(params.error).toBe("access_denied");
    expect(params.code).toBeNull();
  });

  it("flags a legacy implicit-flow fragment instead of trusting it", () => {
    const params = parseCallback(`${REDIRECT_TO}#access_token=eyJhbGciOiJIUzI1NiJ9.xyz`);
    expect(params.legacyTokenInFragment).toBe(true);
    expect(params.code).toBeNull();
  });

  it("rejects an unparseable callback URL", () => {
    expect(() => parseCallback("not a url")).toThrow(AllfatherOAuthError);
  });
});

describe("stripOAuthParams", () => {
  it("removes every sensitive parameter and the fragment, keeping the rest", () => {
    const cleaned = stripOAuthParams(`${REDIRECT_TO}&state=abc&code=the-code&error_description=x#access_token=secret`);
    expect(cleaned).not.toContain("code");
    expect(cleaned).not.toContain("state");
    expect(cleaned).not.toContain("access_token");
    expect(cleaned).not.toContain("error");
    expect(new URL(cleaned).searchParams.get("auth")).toBe("allfather");
  });

  it("is safe on malformed input", () => {
    expect(stripOAuthParams("not a url")).toBe("not a url");
  });
});

/* ------------------------------------------------------------------ *
 * Full callback
 * ------------------------------------------------------------------ */

describe("completeAllfatherCallback", () => {
  function exchangeFetch(body: Record<string, unknown>, ok = true, status = 200) {
    return vi.fn(async (_input: RequestInfo | URL, _init?: RequestInit) => {
      void ok;
      return new Response(JSON.stringify(body), {
        status,
        headers: { "content-type": "application/json" },
      });
    });
  }

  async function seed(): Promise<{ authorizeUrl: string; state: string; verifier: string }> {
    const started = await beginAllfatherSignIn({ ...CONFIG, redirectTo: REDIRECT_TO }, deps());
    const verifier = loadTransaction(storage)!.verifier;
    return { ...started, verifier };
  }

  it("exchanges a valid code and clears the pending transaction", async () => {
    const { state, verifier } = await seed();
    const fetchMock = exchangeFetch({ access_token: "supabase-access-token", refresh_token: "rt" });
    const result = await completeAllfatherCallback(
      { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}&code=the-code` },
      { ...deps(), fetch: fetchMock as unknown as typeof fetch },
    );

    expect(result.accessToken).toBe("supabase-access-token");
    expect(storage.getItem(ALLFATHER_PKCE_STORAGE_KEY)).toBeNull();

    const [url, init] = fetchMock.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe(`${SUPABASE_URL}/auth/v1/token?grant_type=pkce`);
    expect(JSON.parse(String(init.body))).toEqual({ auth_code: "the-code", code_verifier: verifier });
    const headers = init.headers as Record<string, string>;
    expect(headers.apikey).toBe(PUBLISHABLE_KEY);
    expect(headers.authorization).toBe(`Bearer ${PUBLISHABLE_KEY}`);
    // No client secret is ever used by the browser exchange.
    expect(headers.authorization).not.toMatch(/service_role|secret/i);
    expect(Object.keys(headers)).not.toContain("apikey_secret");
  });

  it("never echoes the authorization code or token to the console", async () => {
    const { state } = await seed();
    const spies = (["log", "warn", "error", "info"] as const).map((name) => vi.spyOn(console, name).mockImplementation(() => {}));
    await completeAllfatherCallback(
      { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}&code=the-code` },
      { ...deps(), fetch: exchangeFetch({ access_token: "supabase-access-token" }) as unknown as typeof fetch },
    );
    for (const spy of spies) {
      expect(JSON.stringify(spy.mock.calls)).not.toContain("the-code");
      expect(JSON.stringify(spy.mock.calls)).not.toContain("supabase-access-token");
    }
  });

  it("treats a replayed callback as a missing pending sign-in", async () => {
    const { state } = await seed();
    const fetchMock = exchangeFetch({ access_token: "supabase-access-token" });
    const first = { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}&code=the-code` };
    await completeAllfatherCallback(first, { ...deps(), fetch: fetchMock as unknown as typeof fetch });

    await expect(
      completeAllfatherCallback(first, { ...deps(), fetch: fetchMock as unknown as typeof fetch }),
    ).rejects.toMatchObject({ code: "STATE_MISSING" });
    // Only one exchange was attempted.
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it("rejects a mismatched state and burns the pending transaction", async () => {
    await seed();
    const fetchMock = exchangeFetch({ access_token: "supabase-access-token" });
    await expect(
      completeAllfatherCallback(
        { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=attacker-supplied&code=the-code` },
        { ...deps(), fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "STATE_MISMATCH" });
    expect(fetchMock).not.toHaveBeenCalled();
    expect(storage.getItem(ALLFATHER_PKCE_STORAGE_KEY)).toBeNull();
  });

  it("rejects a state that was never persisted", async () => {
    await expect(
      completeAllfatherCallback({ ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=x&code=the-code` }, {
        ...deps(),
        fetch: exchangeFetch({ access_token: "t" }) as unknown as typeof fetch,
      }),
    ).rejects.toMatchObject({ code: "STATE_MISSING" });
  });

  it("rejects an expired pending sign-in", async () => {
    const { state } = await seed();
    clock += STATE_TTL_MS + 1;
    const fetchMock = exchangeFetch({ access_token: "supabase-access-token" });
    await expect(
      completeAllfatherCallback(
        { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}&code=the-code` },
        { ...deps(), fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "STATE_EXPIRED" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("reports a provider denial without exchanging anything", async () => {
    const { state } = await seed();
    const fetchMock = exchangeFetch({ access_token: "supabase-access-token" });
    await expect(
      completeAllfatherCallback(
        { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}&error=access_denied` },
        { ...deps(), fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "PROVIDER_DENIED" });
    expect(fetchMock).not.toHaveBeenCalled();
    expect(storage.getItem(ALLFATHER_PKCE_STORAGE_KEY)).toBeNull();
  });

  it("refuses a legacy implicit-flow fragment instead of accepting the token", async () => {
    const { state } = await seed();
    const fetchMock = exchangeFetch({ access_token: "supabase-access-token" });
    await expect(
      completeAllfatherCallback(
        { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}#access_token=eyJhbGciOiJIUzI1NiJ9.x.y` },
        { ...deps(), fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "MALFORMED_CALLBACK" });
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("reports a callback with neither code nor error as malformed", async () => {
    await expect(
      completeAllfatherCallback({ ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=whatever` }, deps()),
    ).rejects.toMatchObject({ code: "MALFORMED_CALLBACK" });
  });

  it("clears the pending transaction even when the exchange fails", async () => {
    const { state } = await seed();
    const fetchMock = exchangeFetch({ error: "invalid_grant", error_description: "the-code was used" }, false, 400);
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => {});

    await expect(
      completeAllfatherCallback(
        { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}&code=the-code` },
        { ...deps(), fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "EXCHANGE_FAILED" });

    expect(storage.getItem(ALLFATHER_PKCE_STORAGE_KEY)).toBeNull();
    // The provider's response body (which echoes the code) is never surfaced.
    expect(JSON.stringify(errorLog.mock.calls)).not.toContain("the-code");
  });

  it("maps a network failure to a generic exchange error", async () => {
    const { state } = await seed();
    const fetchMock = vi.fn(async () => {
      throw new Error("network down: https://example-project.supabase.co/auth/v1/token");
    });
    await expect(
      completeAllfatherCallback(
        { ...CONFIG, callbackUrl: `${REDIRECT_TO}&state=${state}&code=the-code` },
        { ...deps(), fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "EXCHANGE_FAILED" });
  });

  it("refuses to run when Supabase is not configured", async () => {
    await expect(
      completeAllfatherCallback(
        { supabaseUrl: "", publishableKey: "", callbackUrl: `${REDIRECT_TO}&code=the-code` },
        deps(),
      ),
    ).rejects.toMatchObject({ code: "MISSING_CONFIG" });
  });
});

/* ------------------------------------------------------------------ *
 * exchangeCodeForSession
 * ------------------------------------------------------------------ */

describe("exchangeCodeForSession", () => {
  it("rejects a non-2xx response without leaking the body", async () => {
    const fetchMock = vi.fn(
      async () => new Response(JSON.stringify({ msg: "invalid code: the-code", error_code: "bad_code" }), { status: 403 }),
    );
    const errorLog = vi.spyOn(console, "error").mockImplementation(() => {});
    await expect(
      exchangeCodeForSession(
        { ...CONFIG, authCode: "the-code", codeVerifier: "verifier" },
        { fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "EXCHANGE_FAILED" });
    expect(JSON.stringify(errorLog.mock.calls)).not.toContain("the-code");
  });

  it("rejects a response with no access token", async () => {
    const fetchMock = vi.fn(async () => new Response(JSON.stringify({ refresh_token: "rt" }), { status: 200 }));
    await expect(
      exchangeCodeForSession(
        { ...CONFIG, authCode: "the-code", codeVerifier: "verifier" },
        { fetch: fetchMock as unknown as typeof fetch },
      ),
    ).rejects.toMatchObject({ code: "EXCHANGE_FAILED" });
  });

  it("handles a missing refresh token", async () => {
    const fetchMock = vi.fn(
      async () => new Response(JSON.stringify({ access_token: "at" }), { status: 200 }),
    );
    const result = await exchangeCodeForSession(
      { ...CONFIG, authCode: "the-code", codeVerifier: "verifier" },
      { fetch: fetchMock as unknown as typeof fetch },
    );
    expect(result).toEqual({ accessToken: "at", refreshToken: null });
  });

  it("refuses without a code or verifier", async () => {
    await expect(
      exchangeCodeForSession({ ...CONFIG, authCode: "", codeVerifier: "v" }),
    ).rejects.toMatchObject({ code: "MALFORMED_CALLBACK" });
  });
});

describe("resolveAllfatherRedirectTo", () => {
  it("keeps the local Admin shell callback on its configured Vite origin", () => {
    expect(resolveAllfatherRedirectTo("http://localhost:5174", "/login", false))
      .toBe("http://localhost:5174/login");
  });

  it("refuses to start local OAuth from a different localhost port", () => {
    expect(() => resolveAllfatherRedirectTo("http://localhost:3000", "/login", false))
      .toThrow(expect.objectContaining({ code: "MISSING_CONFIG" }));
  });

  it("still allows the Windows Tauri development origin", () => {
    expect(resolveAllfatherRedirectTo("http://tauri.localhost", "/login", false))
      .toBe("http://tauri.localhost/login");
  });
});

/* ------------------------------------------------------------------ *
 * Transaction storage
 * ------------------------------------------------------------------ */

describe("transaction storage", () => {
  it("survives a corrupted stored value instead of throwing", () => {
    storage.setItem(ALLFATHER_PKCE_STORAGE_KEY, "{not json");
    expect(loadTransaction(storage)).toBeNull();
    storage.setItem(ALLFATHER_PKCE_STORAGE_KEY, JSON.stringify({ verifier: 1 }));
    expect(loadTransaction(storage)).toBeNull();
  });

  it("round-trips a transaction and clears it", () => {
    saveTransaction(storage, { verifier: "v", state: "s", createdAt: 5 });
    expect(loadTransaction(storage)).toEqual({ verifier: "v", state: "s", createdAt: 5 });
    clearTransaction(storage);
    expect(loadTransaction(storage)).toBeNull();
  });
});
