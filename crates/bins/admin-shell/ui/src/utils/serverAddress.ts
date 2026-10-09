const ENVIRONMENT_KEY = "onyx_backend_environment";
const LOCAL_ADDRESS_KEY = "onyx_local_backend_address";
const CLOUD_ADDRESS_KEY = "onyx_cloud_backend_address";
const LEGACY_ADDRESS_KEY = "onyx_admin_server_address";
const LOCAL_DEFAULT = "http://127.0.0.1:3000";
const CLOUD_DEFAULT = "https://onyx-api-docker.onrender.com";
export type BackendEnvironment = "local" | "cloud";

export function getBackendEnvironment(): BackendEnvironment {
  return localStorage.getItem(ENVIRONMENT_KEY) === "local" ? "local" : "cloud";
}
export function setBackendEnvironment(environment: BackendEnvironment): void {
  localStorage.setItem(ENVIRONMENT_KEY, environment);
}
export function getBackendAddress(environment: BackendEnvironment = getBackendEnvironment()): string {
  const key = environment === "local" ? LOCAL_ADDRESS_KEY : CLOUD_ADDRESS_KEY;
  const stored = localStorage.getItem(key);
  if (stored?.trim()) return stored;
  if (environment === "local") {
    const legacy = localStorage.getItem(LEGACY_ADDRESS_KEY);
    if (legacy?.trim()) return legacy;
    return LOCAL_DEFAULT;
  }
  return import.meta.env.VITE_API_BASE ?? CLOUD_DEFAULT;
}
export function getServerAddress(): string {
  return getBackendAddress();
}
export function setServerAddress(address: string): void {
  const trimmed = address.trim().replace(/\/+$/, "");
  localStorage.setItem(getBackendEnvironment() === "local" ? LOCAL_ADDRESS_KEY : CLOUD_ADDRESS_KEY, trimmed);
  localStorage.setItem(LEGACY_ADDRESS_KEY, trimmed);
}
export function hasStoredServerAddress(): boolean {
  return localStorage.getItem(getBackendEnvironment() === "local" ? LOCAL_ADDRESS_KEY : CLOUD_ADDRESS_KEY) !== null;
}
export function isPlausibleServerAddress(address: string): boolean {
  const trimmed = address.trim();
  if (!trimmed) return false;
  return /^https?:\/\/.+/i.test(trimmed);
}
function isLoopbackHost(hostname: string): boolean {
  return hostname === "127.0.0.1" || hostname === "localhost" || hostname === "::1";
}
export function isSecureEnoughForProduction(address: string): boolean {
  if (!import.meta.env.PROD) return true;
  let parsed: URL;
  try { parsed = new URL(address); } catch { return false; }
  if (parsed.protocol === "https:") return true;
  return parsed.protocol === "http:" && isLoopbackHost(parsed.hostname);
}
