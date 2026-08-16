import { useEffect, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import { EventStream } from '../api/events';
import { useAuthStore } from '../stores/authStore';
import { useNotificationStore } from '../stores/notificationStore';

export function useEventStream() {
  const token = useAuthStore((state) => state.accessToken);
  const organizationId = useAuthStore((state) => state.user?.organization_id);
  const markEvent = useNotificationStore((state) => state.markEvent);
  const client = useQueryClient();
  const [status, setStatus] = useState<'connecting' | 'connected' | 'disconnected'>('disconnected');

  useEffect(() => {
    if (!token || !organizationId) return;
    const stream = new EventStream({
      token,
      filter: { organization_id: organizationId },
      onStatus: (nextStatus) => {
        setStatus(nextStatus);
        if (nextStatus === 'connected') void client.invalidateQueries();
      },
      onResync: () => void client.invalidateQueries(),
      onEvent: (event) => {
        markEvent();
        const type = event.aggregate_ref.type;
        void client.invalidateQueries({ queryKey: [`${type}.list`] });
        void client.invalidateQueries({ queryKey: ['dashboard.summary'] });
      },
    });
    stream.connect();
    return () => stream.close();
  }, [client, markEvent, organizationId, token]);

  return status;
}
