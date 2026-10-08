/**
 * Web Push plumbing for the observer PWA (MIGRATION_PLAN Phase 3.2).
 *
 * The service worker (`public/sw.js`, generated from
 * `scripts/sw.template.js`) already displays `push` events and routes
 * `notificationclick` to the payload's target view; this module owns the
 * page-side lifecycle: requesting permission, creating the browser
 * `PushSubscription`, persisting it server-side via the Phase 1.2
 * `POST/DELETE /api/push/subscriptions` routes, and remembering the
 * subscription id locally so the server row can be removed on opt-out.
 *
 * Read-only by construction: registering a subscription is how a
 * read-class observer *receives* notifications — it never sends one.
 */

import { observerApi } from '../api/onyx';

const STORAGE_KEY = 'onyx_observer_push';

export type PushFailureKind = 'unsupported' | 'unconfigured' | 'denied' | 'unavailable';

export class PushSetupError extends Error {
  readonly kind: PushFailureKind;

  constructor(kind: PushFailureKind, message: string) {
    super(message);
    this.name = 'PushSetupError';
    this.kind = kind;
  }
}

/** The subset of a push session the client needs to remember across reloads. */
export interface PushSubscriptionRecord {
  subscriptionId: string;
  endpoint: string;
}

export function isPushSupported(): boolean {
  return (
    typeof navigator !== 'undefined' &&
    'serviceWorker' in navigator &&
    'window' in globalThis &&
    'PushManager' in window &&
    'Notification' in window
  );
}

/** The server's VAPID *public* key, published as `VITE_VAPID_PUBLIC_KEY`. */
export function getVapidKey(): string | null {
  const key = import.meta.env?.VITE_VAPID_PUBLIC_KEY;
  return typeof key === 'string' && key.length > 0 ? key : null;
}

export function readStoredPush(): PushSubscriptionRecord | null {
  const raw = localStorage.getItem(STORAGE_KEY);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as PushSubscriptionRecord;
  } catch {
    localStorage.removeItem(STORAGE_KEY);
    return null;
  }
}

export function writeStoredPush(record: PushSubscriptionRecord): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(record));
}

export function clearStoredPush(): void {
  localStorage.removeItem(STORAGE_KEY);
}

/** Base64url (no padding) → bytes, per the Web Push `applicationServerKey`. */
export function urlBase64ToUint8Array(base64Url: string): Uint8Array<ArrayBuffer> {
  const normalized = base64Url.replaceAll('-', '+').replaceAll('_', '/');
  const padded = normalized + '='.repeat((4 - (normalized.length % 4)) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

/** Bytes → base64url (no padding) — the wire shape the API validates. */
export function urlBase64Encode(input: ArrayBuffer | Uint8Array): string {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replaceAll('=', '');
}

/** Registers (idempotently) the service worker the push subscription lives on. */
export async function ensureRegistration(): Promise<ServiceWorkerRegistration> {
  if (!('serviceWorker' in navigator)) {
    throw new PushSetupError('unsupported', 'Service workers are not supported in this browser.');
  }
  try {
    return await navigator.serviceWorker.register('/sw.js', {
      scope: '/',
      updateViaCache: 'none',
    });
  } catch {
    throw new PushSetupError('unavailable', 'The service worker could not be registered.');
  }
}

/**
 * Opts in: asks permission, creates/uses the browser subscription, and
 * registers it with the server. Returns the server-side subscription id so
 * `disable` can DELETE the exact row.
 */
export async function subscribeToPush(): Promise<PushSubscriptionRecord> {
  const vapidKey = getVapidKey();
  if (!vapidKey) {
    throw new PushSetupError('unconfigured', 'No VAPID public key is configured for this deployment.');
  }
  if (!isPushSupported()) {
    throw new PushSetupError('unsupported', 'Push notifications are not supported in this browser.');
  }

  const permission = await Notification.requestPermission();
  if (permission !== 'granted') {
    throw new PushSetupError('denied', 'Notification permission was not granted.');
  }

  const registration = await ensureRegistration();
  let subscription = await registration.pushManager.getSubscription();
  if (!subscription) {
    subscription = await registration.pushManager.subscribe({
      userVisibleOnly: true,
      applicationServerKey: urlBase64ToUint8Array(vapidKey),
    });
  }

  const p256dh = urlBase64Encode(subscription.getKey('p256dh') ?? new Uint8Array());
  const auth = urlBase64Encode(subscription.getKey('auth') ?? new Uint8Array());
  if (!p256dh || !auth) {
    throw new PushSetupError('unavailable', 'The browser returned an incomplete push subscription.');
  }

  const response = await observerApi.registerPush({
    endpoint: subscription.endpoint,
    p256dh,
    auth,
    platform: 'pwa',
  });

  return { subscriptionId: response.data.id, endpoint: subscription.endpoint };
}

/**
 * Opts out: removes the server row for the stored subscription id, then
 * best-effort unsubscribes the local browser subscription. The local
 * unsubscribe always runs even if the server call fails.
 */
export async function unsubscribeFromPush(subscriptionId: string): Promise<void> {
  try {
    await observerApi.unregisterPush(subscriptionId);
  } catch {
    /* the browser-side unsubscribe below still runs; the server row is best-effort */
  }
  if ('serviceWorker' in navigator) {
    try {
      const registration = await navigator.serviceWorker.register('/sw.js');
      const subscription = await registration.pushManager.getSubscription();
      if (subscription) await subscription.unsubscribe();
    } catch {
      /* browser-side best effort */
    }
  }
}