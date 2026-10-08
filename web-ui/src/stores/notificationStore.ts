import { create } from 'zustand';
import type { NotificationProjection } from '../types/query';

interface NotificationState {
  selectedId: string | null;
  lastEventAt: string | null;
  select: (id: string | null) => void;
  markEvent: () => void;
  unreadCount: (items: NotificationProjection[]) => number;
}

export const useNotificationStore = create<NotificationState>((set) => ({
  selectedId: null,
  lastEventAt: null,
  select: (selectedId) => set({ selectedId }),
  markEvent: () => set({ lastEventAt: new Date().toISOString() }),
  unreadCount: (items) => items.filter((item) => item.status === 'unacknowledged').length,
}));
