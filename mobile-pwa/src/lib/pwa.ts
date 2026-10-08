/**
 * PWA registration (Phase 2.2). The manifest is a plain static
 * `public/manifest.webmanifest` and the worker is generated at install
 * time (`scripts/postinstall.mjs`) — no vite-plugin-pwa, per Phase 2.2's
 * "-disable for PWAKit (use plain Vite PWA manifest)" decision.
 *
 * Phase 3.2 (Web Push) will extend this module with the `PushManager`
 * registration / `registrationKey` producers; today it only concerns the
 * service worker lifecycle.
 */

export async function registerServiceWorker(): Promise<ServiceWorkerRegistration | null> {
  if (!('serviceWorker' in navigator)) return null;
  if (import.meta.env.DEV && !import.meta.env.VITE_ENABLE_SW_DEV) return null;
  try {
    return await navigator.serviceWorker.register('/sw.js', {
      scope: '/',
      updateViaCache: 'none',
    });
  } catch (error) {
    console.error('[pwa] service worker registration failed:', error);
    return null;
  }
}