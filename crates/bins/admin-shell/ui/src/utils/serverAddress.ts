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
