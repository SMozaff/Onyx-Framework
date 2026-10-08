import { create } from 'zustand';
import {
  clearStoredPush,
  getVapidKey,
  isPushSupported,
  PushSetupError,
  readStoredPush,
  subscribeToPush,
  unsubscribeFromPush,
  writeStoredPush,
} from '../push/push';

export type PushStatus =
  | 'idle'
  | 'unsupported'
  | 'unconfigured'
  | 'denied'
  | 'subscribed'
  | 'error';

interface PushState {
  status: PushStatus;
  subscriptionId: string | null;
  endpoint: string | null;
  pending: boolean;
  message: string | null;
  boot: () => Promise<void>;
  enable: () => Promise<void>;
  disable: () => Promise<void>;
}

const FAILURE_TO_STATUS: Record<PushSetupError['kind'], PushStatus> = {
  unsupported: 'unsupported',
  unconfigured: 'unconfigured',
  denied: 'denied',
  unavailable: 'error',
};

export const usePushStore = create<PushState>((set, get) => ({
  status: 'idle',
  subscriptionId: null,
  endpoint: null,
  pending: false,
  message: null,

  /** Reconciles capability + config + the persisted session on mount. */
  boot: async () => {
    if (!isPushSupported()) {
      set({ status: 'unsupported', message: null });
      return;
    }
    if (!getVapidKey()) {
      set({ status: 'unconfigured', message: null });
      return;
    }
    const stored = readStoredPush();
    if (stored) {
      set({ status: 'subscribed', subscriptionId: stored.subscriptionId, endpoint: stored.endpoint });
    }
  },

  enable: async () => {
    if (get().pending) return;
    set({ pending: true, message: null });
    try {
      const record = await subscribeToPush();
      writeStoredPush(record);
      set({
        status: 'subscribed',
        subscriptionId: record.subscriptionId,
        endpoint: record.endpoint,
        pending: false,
      });
    } catch (error) {
      const kind = error instanceof PushSetupError ? error.kind : 'unavailable';
      set({
        status: FAILURE_TO_STATUS[kind] ?? 'error',
        message: error instanceof Error ? error.message : 'Push setup failed.',
        pending: false,
      });
    }
  },

  disable: async () => {
    const { subscriptionId, pending } = get();
    if (!subscriptionId || pending) return;
    set({ pending: true, message: null });
    try {
      await unsubscribeFromPush(subscriptionId);
    } finally {
      clearStoredPush();
      set({ status: 'idle', subscriptionId: null, endpoint: null, pending: false });
    }
  },
}));