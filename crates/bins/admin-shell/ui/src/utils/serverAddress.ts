const SERVER_ADDRESS_KEY = "onyx_admin_server_address";

/**
 * The default server address, used only when nothing has been saved
 * yet. Kept as `VITE_API_BASE` (build-time override) falling back to
 * localhost, matching the previous hardcoded behavior in
 * `api/client.ts` — so a fresh install with no settings configured
 * still behaves exactly as before.
 */
const BUILD_TIME_DEFAULT = import.meta.env.VITE_API_BASE ?? "http://127.0.0.1:3000";

/**
 * Reads the user-configured server address from persistent storage
 * (`localStorage`, not `sessionStorage` — unlike the auth session in
 * `utils/auth.ts`, this must survive app restarts; that's the entire
 * point of making it configurable). Falls back to the build-time
 * default if nothing has been saved yet.
 */
export function getServerAddress(): string {
  const stored = localStorage.getItem(SERVER_ADDRESS_KEY);
  return stored && stored.trim().length > 0 ? stored : BUILD_TIME_DEFAULT;
}

/**
 * Saves a new server address. Strips a trailing slash so
 * `http://host:3000/` and `http://host:3000` behave identically when
 * concatenated with request paths elsewhere (`apiClient` always calls
 * paths like `/api/...` with a leading slash).
 */
export function setServerAddress(address: string): void {
  const trimmed = address.trim().replace(/\/+$/, "");
  localStorage.setItem(SERVER_ADDRESS_KEY, trimmed);
}

/**
 * True if the user has explicitly configured an address (as opposed
 * to still running on the build-time default). Used by Settings to
 * show whether a value is "saved" vs. just the default placeholder.
 */
export function hasStoredServerAddress(): boolean {
  return localStorage.getItem(SERVER_ADDRESS_KEY) !== null;
}

/**
 * Very loose validation — just enough to catch obvious typos (empty,
 * no scheme) before saving, without trying to fully validate a URL
 * (ports, IPv6 literals, etc. all vary legitimately here).
 */
export function isPlausibleServerAddress(address: string): boolean {
  const trimmed = address.trim();
  if (trimmed.length === 0) return false;
  return /^https?:\/\/.+/i.test(trimmed);
}

function isLoopbackHost(hostname: string): boolean {
  // `new URL(...)` strips the brackets from an IPv6 literal host, so the
  // bare `::1` form is what actually appears here for "[::1]" input.
  return hostname === "127.0.0.1" || hostname === "localhost" || hostname === "::1";
}

/**
 * Audit finding H4(b): a plaintext `http://` connection to anything other
 * than this same machine sends credentials (the login password, then every
 * bearer token) over the network in the clear — trivially interceptable on
 * shared Wi-Fi, a compromised router, or any on-path network position. Only
 * loopback traffic (`http://127.0.0.1`, `http://localhost`) never actually
 * leaves the machine, so it is exempt; every other address must use
 * `https://`.
 *
 * Gated on `import.meta.env.PROD`, Vite's own, already-real build-mode
 * flag — not a new `ONYX_ENV`-style variable invented for this. This repo
 * has no existing client-side equivalent to the server's `ONYX_ENV`
 * (confirmed: no such mechanism exists anywhere in `admin-shell/ui`,
 * checked directly rather than assumed), but it does not need one: `npm
 * run build` (what `tauri build` invokes to produce the actual shipped
 * app; see `package.json`'s `build` script) always runs `vite build`,
 * which sets `import.meta.env.PROD = true` unconditionally, while `npm run
 * dev` / `tauri dev` set `import.meta.env.DEV = true` instead. That
 * distinction already exactly tracks "is this the real, distributed
 * application or a local development run" — introducing a parallel
 * `ONYX_ENV`-equivalent would just be a second flag carrying the same
 * meaning as one Vite already provides for free.
 */
export function isSecureEnoughForProduction(address: string): boolean {
  if (!import.meta.env.PROD) return true;
  let parsed: URL;
  try {
    parsed = new URL(address);
  } catch {
    return false;
  }
  if (parsed.protocol === "https:") return true;
  if (parsed.protocol === "http:") return isLoopbackHost(parsed.hostname);
  return false;
}
