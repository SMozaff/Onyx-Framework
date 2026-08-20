import StatusBadge from '../../components/StatusBadge';
import { deriveProjectionState, Freshness, ProjectionStatePanel } from '../../components/ProjectionState';
import { useAcknowledgeNotification } from '../../hooks/useCommand';
import { useOnyxQuery } from '../../hooks/useQuery';
import type { NotificationProjection } from '../../types/query';

export default function NotificationsPage() {
  const query = useOnyxQuery<NotificationProjection>('notification.list');
  const state = deriveProjectionState(query);
  const acknowledge = useAcknowledgeNotification();
  const notifications = query.data?.data ?? [];
  const unacknowledged = notifications.filter((item) => item.status === 'unacknowledged').length;

  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">Delivery center</p><h1>Notifications</h1><p>Acknowledgment is sent to the server immediately. Failed commands are never queued locally.</p></div><div className="topbar-actions"><Freshness state={state} /><button type="button" className="button-secondary" onClick={() => void query.refetch()} disabled={query.isFetching}>Refresh</button></div></header>
    {state.kind === 'loading' ? <div className="skeleton-list" aria-label="Loading notifications" /> : state.kind === 'unavailable' ? <ProjectionStatePanel resource="Notifications" state={state} /> : <>{state.kind === 'stale' && <ProjectionStatePanel resource="Notifications" state={state} compact />}<section className="panel"><div className="panel-heading"><h2>Operational alerts</h2><span>{unacknowledged} unacknowledged</span></div>{state.kind === 'empty' ? <div className="empty-state"><h3>No notifications</h3><p>No notification projections are currently available for this organization.</p></div> : <div className="card-list">{notifications.map((item) => <article className="notification-card" key={item.id}><div className="notification-content"><div className="card-title-row"><h3>{item.title}</h3><StatusBadge status={item.status} /></div><p>{item.message}</p><small>{new Date(item.created_at).toLocaleString()} · {item.source_type}</small></div>{item.status === 'unacknowledged' ? <button type="button" className="button-primary" disabled={acknowledge.isPending} onClick={() => acknowledge.mutate(item)}>{acknowledge.isPending ? 'Sending…' : 'Acknowledge'}</button> : <span className="completed-label">Acknowledged</span>}</article>)}</div>}</section></>}
  </div>;
}
