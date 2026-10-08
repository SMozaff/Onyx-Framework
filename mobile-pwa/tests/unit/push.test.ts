import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  clearStoredPush,
  getVapidKey,
  isPushSupported,
  PushSetupError,
  readStoredPush,
  urlBase64Encode,
  urlBase64ToUint8Array,
  writeStoredPush,
} from '../../src/push/push';
import { usePushStore } from '../../src/stores/pushStore';

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  usePushStore.setState({ status: 'idle', subscriptionId: null, endpoint: null, pending: false, message: null });
  vi.unstubAllEnvs();
});

describe('push key helpers', () => {
  it('decodes base64url no-pad and round-trips through encode', () => {
    const bytes = urlBase64ToUint8Array('cGFyZW50a2V5');
    expect(Array.from(bytes)).toEqual([...Buffer.from('parentkey', 'utf8')]);
    expect(urlBase64Encode(bytes)).toBe('cGFyZW50a2V5');
  });

  it('encodes ArrayBuffer input the same way', () => {
    const arrayBuffer = new TextEncoder().encode('authsecret').buffer as ArrayBuffer;
    expect(urlBase64Encode(arrayBuffer)).toBe('YXV0aHNlY3JldA');
  });

  it('is unsupported in the jsdom test environment', () => {
    expect(isPushSupported()).toBe(false);
  });

  it('exposes the VAPID key only when configured', () => {
    expect(getVapidKey()).toBeNull();
    vi.stubEnv('VITE_VAPID_PUBLIC_KEY', 'test-public-key');
    expect(getVapidKey()).toBe('test-public-key');
  });
});

describe('push session persistence', () => {
  it('round-trips the stored subscription and clears it', () => {
    expect(readStoredPush()).toBeNull();
    writeStoredPush({ subscriptionId: 'sub-1', endpoint: 'https://push.example.test/x' });
    expect(readStoredPush()).toEqual({ subscriptionId: 'sub-1', endpoint: 'https://push.example.test/x' });
    clearStoredPush();
    expect(readStoredPush()).toBeNull();
  });

  it('tolerates corrupt stored JSON', () => {
    localStorage.setItem('onyx_observer_push', '{not json');
    expect(readStoredPush()).toBeNull();
    expect(localStorage.getItem('onyx_observer_push')).toBeNull();
  });
});

describe('PushSetupError', () => {
  it('carries its failure kind', () => {
    const error = new PushSetupError('denied', 'Notification permission was not granted.');
    expect(error.kind).toBe('denied');
    expect(error.message).toContain('permission');
  });
});