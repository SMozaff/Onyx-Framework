const CLOUD_API_BASE = "https://onyx-framework.soheil-mozaffari.workers.dev";

/** Single supported API endpoint for every Admin client. */
export function getServerAddress(): string {
  return CLOUD_API_BASE;
}

/** Kept as a compatibility helper for existing imports; endpoint is immutable. */
export function getBackendAddress(): string {
  return CLOUD_API_BASE;
}

export function isSecureEnoughForProduction(address: string): boolean {
  try {
    return new URL(address).protocol === "https:" && address.replace(/\/+$/, "") === CLOUD_API_BASE;
  } catch {
    return false;
  }
}
