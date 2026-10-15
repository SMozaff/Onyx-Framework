/**
 * All-Father Google sign-in for the Admin shell — Authorization Code Flow with PKCE.
 *
 * Why this replaces the implicit flow
 * -----------------------------------
 * The previous implementation navigated to Supabase's `/auth/v1/authorize`
 * with `flow_type=implicit` and read an `access_token` straight out of the URL
 * fragment. That puts a bearer credential in the address bar, in browser
 * history, in referrer headers and in any URL-forwarding proxy, and offers no
 * CSRF binding at all.
 *
 * This module drives the Authorization Code Flow with PKCE instead:
 *
 *  1. `beginAllfatherSignIn` creates a 32-byte random verifier, its S256
 *     challenge, and a separate random `state` nonce. Only the nonce and the
 *     verifier are stored, in this tab's `sessionStorage`.
 *  2. The user is sent to `/auth/v1/authorize` with `code_challenge` +
 *     `code_challenge_method=s256` — the exact query shape Supabase's own
 *     `@supabase/auth-js` client sends (see `_getUrlForProvider`), so no
 *     client secret and no publishable-key-only assumption change.
 *  3. Supabase redirects back to the registered `redirect_to` with a single-use
 *     `?code=...`. `completeAllfatherCallback` checks the `state` nonce, then
 *     exchanges the code for the session with
 *     `POST /auth/v1/token?grant_type=pkce` using only the publishable key.
 *
 * On `state`
 * ----------
 * OAuth `state` is not passed to `/authorize` here on purpose: Supabase Auth
 * owns that parameter internally and rejects a caller-supplied value
 * (supabase/auth#1548). The CSRF nonce therefore travels inside the
 * allow-listed `redirect_to` URL, which is the approach Supabase documents for
 * application state, and it is validated on the way back exactly like a
 * conventional `state`.
 *
 * Everything in this file is side-effect-free until an exported function is
 * called, and every external dependency (`crypto`, `storage`, `fetch`, `now`)
 * is injectable so the flow can be regression-tested without a browser.
 */

/** sessionStorage key holding the pending PKCE transaction (verifier + state). */
export const ALLFATHER_PKCE_STORAGE_KEY = "onyx_admin_allfather_pkce";

/** `redirect_to` marker that selects the All-Father sign-in path on return. */
export const ALLFATHER_AUTH_QUERY_KEY = "auth";
export const ALLFATHER_AUTH_QUERY_VALUE = "allfather";
/** Query key carrying our CSRF nonce inside `redirect_to`. */
export const ALLFATHER_STATE_QUERY_KEY = "state";

export const SUPABASE_AUTH_PROVIDER = "google";
/** RFC 7636 lower-case S256, matching Supabase's own client. */
export const PKCE_CHALLENGE_METHOD = "s256";
/** How long a pending sign-in may sit between redirect and callback. */
export const STATE_TTL_MS = 10 * 60 * 1000;

export type AllfatherOAuthErrorCode =
  | "MISSING_CONFIG"
  | "PROVIDER_DENIED"
  | "MALFORMED_CALLBACK"
  | "STATE_MISSING"
  | "STATE_MISMATCH"
  | "STATE_EXPIRED"
  | "EXCHANGE_FAILED";

/**
 * Failure modes are exposed to the caller only as stable codes. Messages are
 * generic on purpose: no authorization code, verifier, state, access token or
 * provider error text is ever carried out of this module.
 */
export class AllfatherOAuthError extends Error {
  readonly code: AllfatherOAuthErrorCode;

  constructor(code: AllfatherOAuthErrorCode, message: string) {
    super(message);
    this.name = "AllfatherOAuthError";
    this.code = code;
  }
}

export interface StorageLike {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export interface OAuthDeps {
  crypto?: Crypto;
  storage?: StorageLike;
  fetch?: typeof fetch;
  now?: () => number;
}

export interface AllfatherConfig {
  supabaseUrl: string;
  publishableKey: string;
}

export interface PkceTransaction {
  verifier: string;
  state: string;
  createdAt: number;
}

export interface CallbackParams {
  /** Single-use authorization code, read from the query only. */
  code: string | null;
  state: string | null;
  /** Provider denial/cancellation, from the query or the fragment. */
  error: string | null;
  /**
   * True when a legacy implicit-flow response (`#access_token=...`) reached the
   * PKCE callback. Such a response is always refused rather than processed.
   */
  legacyTokenInFragment: boolean;
}

function requireCrypto(deps: OAuthDeps = {}): Crypto {
  const cryptoImpl = deps.crypto ?? (globalThis.crypto as Crypto | undefined);
  if (!cryptoImpl) throw new AllfatherOAuthError("MISSING_CONFIG", "WebCrypto is unavailable");
  return cryptoImpl;
}

function defaultStorage(): StorageLike {
  const storage = (globalThis as { sessionStorage?: StorageLike }).sessionStorage;
  if (!storage) {
    throw new AllfatherOAuthError("MISSING_CONFIG", "sessionStorage is unavailable in this environment");
  }
  return storage;
}

function requireConfig(config: AllfatherConfig): { supabaseUrl: string; publishableKey: string } {
  const supabaseUrl = (config.supabaseUrl ?? "").replace(/\/+$/, "");
  const publishableKey = (config.publishableKey ?? "").trim();
  if (!supabaseUrl || !publishableKey) {
    throw new AllfatherOAuthError("MISSING_CONFIG", "Supabase is not configured in this build");
  }
  return { supabaseUrl, publishableKey };
}

/** base64url without padding, per RFC 7636 / RFC 4648 §5. */
export function base64UrlEncode(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

/** 32 random bytes → 43 base64url chars: the RFC 7636 recommended verifier. */
export function generateCodeVerifier(deps: OAuthDeps = {}): string {
  const bytes = new Uint8Array(32);
  requireCrypto(deps).getRandomValues(bytes);
  return base64UrlEncode(bytes);
}

/** BASE64URL-ENCODE(SHA256(ASCII(verifier))) — RFC 7636 §4.2, method S256. */
export async function deriveCodeChallenge(verifier: string, deps: OAuthDeps = {}): Promise<string> {
  if (!verifier) throw new AllfatherOAuthError("MALFORMED_CALLBACK", "missing PKCE verifier");
  const digest = await requireCrypto(deps).subtle.digest("SHA-256", new TextEncoder().encode(verifier));
  return base64UrlEncode(new Uint8Array(digest));
}

/** 16 random bytes of CSRF nonce; unpredictable across the whole flow. */
export function generateState(deps: OAuthDeps = {}): string {
  const bytes = new Uint8Array(16);
  requireCrypto(deps).getRandomValues(bytes);
  return base64UrlEncode(bytes);
}

/**
 * Builds the Supabase authorize URL with the same query shape as
 * `@supabase/auth-js`'s `_getUrlForProvider`: `provider`, `redirect_to`, then
 * the PKCE challenge, then `apikey`. Our CSRF nonce rides inside `redirect_to`.
 */
/**
 * Resolves the callback URI without allowing a packaged build to silently
 * fall back to a local development server. Linux/macOS Tauri uses
 * tauri://localhost; Windows Tauri may use http://tauri.localhost.
 *
 * Supabase Auth must separately allow the resulting URI in its redirect URL
 * configuration. This guard fails closed rather than sending users to a
 * loopback development server such as localhost:3000.
 */
export function resolveAllfatherRedirectTo(
  origin: string,
  pathname: string,
  productionBuild: boolean,
): string {
  let target: URL;
  try {
    target = new URL(pathname, origin);
  } catch {
    throw new AllfatherOAuthError("MISSING_CONFIG", "desktop callback URI is invalid");
  }

  const isLoopbackHost =
    target.hostname === "localhost" ||
    target.hostname === "127.0.0.1" ||
    target.hostname === "[::1]" ||
    target.hostname === "::1";
  const isTauriOrigin =
    (target.protocol === "tauri:" && target.hostname === "localhost") ||
    (target.protocol === "http:" && target.hostname === "tauri.localhost");

  if (
    productionBuild &&
    ((isLoopbackHost && !isTauriOrigin) ||
      (target.protocol !== "https:" && !isTauriOrigin))
  ) {
    throw new AllfatherOAuthError(
      "MISSING_CONFIG",
      "packaged desktop sign-in cannot use a local development callback",
    );
  }

  return `${target.origin}${target.pathname}`;
}

export function buildAuthorizeUrl(
  options: AllfatherConfig & {
    redirectTo: string;
    codeChallenge: string;
    state: string;
    provider?: string;
  },
): string {
  const { supabaseUrl, publishableKey } = requireConfig(options);
  if (!options.redirectTo) throw new AllfatherOAuthError("MISSING_CONFIG", "redirect URI is not configured");

  const redirect = new URL(options.redirectTo);
  redirect.searchParams.set(ALLFATHER_AUTH_QUERY_KEY, ALLFATHER_AUTH_QUERY_VALUE);
  redirect.searchParams.set(ALLFATHER_STATE_QUERY_KEY, options.state);

  const target = new URL(`${supabaseUrl}/auth/v1/authorize`);
  target.searchParams.set("provider", options.provider ?? SUPABASE_AUTH_PROVIDER);
  target.searchParams.set("redirect_to", redirect.toString());
  target.searchParams.set("code_challenge", options.codeChallenge);
  target.searchParams.set("code_challenge_method", PKCE_CHALLENGE_METHOD);
  target.searchParams.set("apikey", publishableKey);
  return target.toString();
}

export function saveTransaction(storage: StorageLike, transaction: PkceTransaction): void {
  storage.setItem(ALLFATHER_PKCE_STORAGE_KEY, JSON.stringify(transaction));
}

export function loadTransaction(storage: StorageLike): PkceTransaction | null {
  const raw = storage.getItem(ALLFATHER_PKCE_STORAGE_KEY);
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<PkceTransaction>;
    if (typeof parsed.verifier !== "string" || typeof parsed.state !== "string") return null;
    return { verifier: parsed.verifier, state: parsed.state, createdAt: Number(parsed.createdAt ?? 0) };
  } catch {
    return null;
  }
}

export function clearTransaction(storage: StorageLike): void {
  storage.removeItem(ALLFATHER_PKCE_STORAGE_KEY);
}

/**
 * Starts a sign-in: persists the verifier + nonce, returns the authorize URL to
 * navigate to. The verifier never leaves this browser tab until it is needed
 * for the code exchange.
 */
export async function beginAllfatherSignIn(
  config: AllfatherConfig & { redirectTo: string; provider?: string },
  deps: OAuthDeps = {},
): Promise<{ authorizeUrl: string; state: string }> {
  // Validate configuration BEFORE persisting anything, so a misconfigured
  // build can never leave a half-finished PKCE transaction behind.
  requireConfig(config);
  const storage = deps.storage ?? defaultStorage();
  const now = deps.now ?? Date.now;
  const verifier = generateCodeVerifier(deps);
  const codeChallenge = await deriveCodeChallenge(verifier, deps);
  const state = generateState(deps);
  saveTransaction(storage, { verifier, state, createdAt: now() });
  return { authorizeUrl: buildAuthorizeUrl({ ...config, codeChallenge, state }), state };
}

/**
 * Reads a callback URL. Only the query is trusted for `code`/`state`; the
 * fragment is inspected *solely* to detect and reject a legacy implicit-flow
 * response.
 */
export function parseCallback(callbackUrl: string): CallbackParams {
  let url: URL;
  try {
    url = new URL(callbackUrl);
  } catch {
    throw new AllfatherOAuthError("MALFORMED_CALLBACK", "callback URL could not be parsed");
  }
  const fragment = url.hash.startsWith("#") ? url.hash.slice(1) : url.hash;
  const fragmentParams = new URLSearchParams(fragment);
  return {
    code: url.searchParams.get("code"),
    state: url.searchParams.get("state"),
    error: url.searchParams.get("error") ?? fragmentParams.get("error"),
    legacyTokenInFragment: fragmentParams.has("access_token"),
  };
}

/**
 * Strips the authorization code and any OAuth state/error parameters from a
 * URL so no credential material is left in the address bar, in history, or in
 * a referrer. Safe to call before any asynchronous work.
 */
export function stripOAuthParams(callbackUrl: string): string {
  try {
    const url = new URL(callbackUrl);
    for (const key of ["code", "state", "error", "error_description", "error_code"]) {
      url.searchParams.delete(key);
    }
    url.hash = "";
    return url.toString();
  } catch {
    return callbackUrl;
  }
}

/**
 * Exchanges the single-use authorization code for a Supabase session using only
 * the publishable (browser-safe) key. Mirrors `POST /auth/v1/token?grant_type=pkce`.
 */
export async function exchangeCodeForSession(
  config: AllfatherConfig & { authCode: string; codeVerifier: string },
  deps: OAuthDeps = {},
): Promise<{ accessToken: string; refreshToken: string | null }> {
  const { supabaseUrl, publishableKey } = requireConfig(config);
  if (!config.authCode || !config.codeVerifier) {
    throw new AllfatherOAuthError("MALFORMED_CALLBACK", "missing authorization code or verifier");
  }
  const fetchImpl = deps.fetch ?? (globalThis.fetch as typeof fetch | undefined);
  if (!fetchImpl) throw new AllfatherOAuthError("MISSING_CONFIG", "fetch is unavailable");

  let response: Response;
  try {
    response = await fetchImpl(`${supabaseUrl}/auth/v1/token?grant_type=pkce`, {
      method: "POST",
      headers: {
        apikey: publishableKey,
        authorization: `Bearer ${publishableKey}`,
        "content-type": "application/json",
        accept: "application/json",
      },
      body: JSON.stringify({ auth_code: config.authCode, code_verifier: config.codeVerifier }),
    });
  } catch {
    throw new AllfatherOAuthError("EXCHANGE_FAILED", "could not reach the identity provider");
  }

  const body = (await response.json().catch(() => ({}))) as { access_token?: unknown; refresh_token?: unknown };
  if (!response.ok || typeof body.access_token !== "string" || body.access_token.length === 0) {
    // Deliberately does not include the response body: providers echo the code
    // in some error payloads, and that must not reach the console or the UI.
    throw new AllfatherOAuthError("EXCHANGE_FAILED", "the authorization code could not be exchanged");
  }
  return {
    accessToken: body.access_token,
    refreshToken: typeof body.refresh_token === "string" ? body.refresh_token : null,
  };
}

/**
 * Validates and consumes a callback, returning the Supabase access token that
 * the Worker then exchanges for an ONYX session.
 *
 * Fail-closed in this order: config → provider denial → malformed response →
 * missing/mismatched/expired state → single-use exchange. The stored
 * transaction is cleared before the network call, so a replayed `code` (or a
 * second tab) can never re-use a verifier.
 */
export async function completeAllfatherCallback(
  config: AllfatherConfig & { callbackUrl: string },
  deps: OAuthDeps = {},
): Promise<{ accessToken: string }> {
  requireConfig(config);
  const storage = deps.storage ?? defaultStorage();
  const now = deps.now ?? Date.now;

  const params = parseCallback(config.callbackUrl);
  if (params.legacyTokenInFragment) {
    clearTransaction(storage);
    throw new AllfatherOAuthError("MALFORMED_CALLBACK", "implicit-flow responses are not accepted");
  }
  if (params.error) {
    clearTransaction(storage);
    throw new AllfatherOAuthError("PROVIDER_DENIED", "the identity provider did not complete sign-in");
  }
  if (!params.code) {
    clearTransaction(storage);
    throw new AllfatherOAuthError("MALFORMED_CALLBACK", "no authorization code was returned");
  }

  const transaction = loadTransaction(storage);
  if (!transaction) {
    throw new AllfatherOAuthError("STATE_MISSING", "no pending sign-in; start again");
  }
  if (!params.state || params.state !== transaction.state) {
    clearTransaction(storage);
    throw new AllfatherOAuthError("STATE_MISMATCH", "sign-in state did not match");
  }
  if (now() - transaction.createdAt > STATE_TTL_MS) {
    clearTransaction(storage);
    throw new AllfatherOAuthError("STATE_EXPIRED", "sign-in request expired; start again");
  }

  // Single use: clear before exchanging so the same code/verifier pair cannot
  // be replayed, even if the exchange fails.
  clearTransaction(storage);

  const { accessToken } = await exchangeCodeForSession(
    {
      supabaseUrl: config.supabaseUrl,
      publishableKey: config.publishableKey,
      authCode: params.code,
      codeVerifier: transaction.verifier,
    },
    deps,
  );
  return { accessToken };
}
