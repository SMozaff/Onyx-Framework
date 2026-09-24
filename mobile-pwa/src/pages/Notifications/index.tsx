import { StatusBadge } from '../../components/StatusBadge';
import { Freshness } from '../../components/Freshness';
import { ProjectionStatePanel } from '../../components/ProjectionStatePanel';
import { PushNotificationsCard } from '../../components/PushNotificationsCard';
import { deriveProjectionState } from '../../components/ProjectionState';
import { useObserverQuery } from '../../hooks/useQuery';
import type { NotificationProjection } from '../../types/query';

export function NotificationsPage() {
  const query = useObserverQuery<NotificationProjection>('notification.list');
  const state = deriveProjectionState(query);
  const notifications = query.data?.data ?? [];
  const unacknowledged = notifications.filter((item) => item.status === 'unacknowledged').length;

  return (
    <div className="space-y-6">
      <header className="flex items-start justify-between">
        <div>
          <p className="text-xs uppercase tracking-wide text-slate-500">Delivery center</p>
          <h2 className="text-lg font-semibold text-slate-900">Notifications</h2>
          <p className="text-sm text-slate-600">
            Observer notifications are ack-only via Web Push installments; this client never
            acknowledges on your behalf.
          </p>
        </div>
        <Freshness state={state} />
      </header>

      {state.kind === 'loading' && <p className="text-sm text-slate-500">Loading…</p>}
      {state.kind === 'unavailable' && (
        <ProjectionStatePanel resource="Notifications" state={state} />
      )}
      {state.kind === 'empty' && (
        <p className="text-sm text-slate-500">No notifications for your organization.</p>
      )}
      {state.kind === 'stale' && (
        <ProjectionStatePanel resource="Notifications" state={state} compact />
      )}

      <PushNotificationsCard />

      {notifications.length > 0 && (
        <>
          <p className="text-xs text-slate-500">
            {unacknowledged} unacknowledged · {notifications.length} total
          </p>
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {notifications.map((item) => (
              <li key={item.id} className="px-4 py-3">
                <div className="flex items-center justify-between gap-3">
                  <p className="font-medium text-slate-900">{item.title}</p>
                  <StatusBadge status={item.status} />
                </div>
                <p className="text-sm text-slate-700">{item.message}</p>
                <p className="mt-1 text-xs text-slate-500">
                  {new Date(item.created_at).toLocaleString()} · {item.source_type}
                </p>
              </li>
            ))}
          </ul>
        </>
      )}
      <footer className="text-xs text-slate-400">
        projection version {query.data?.freshness.projection_version ?? '—'}
      </footer>
    </div>
  );
}