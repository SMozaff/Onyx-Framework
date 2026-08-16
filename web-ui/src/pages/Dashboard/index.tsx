import { Link } from 'react-router-dom';
import { useOnyxQuery } from '../../hooks/useQuery';
import type { DashboardProjection } from '../../types/query';
import StatsGrid from './components/StatsGrid';
import AlertBanner from './components/AlertBanner';
import ActivityFeed from './components/ActivityFeed';

export default function DashboardPage() {
  const query = useOnyxQuery<DashboardProjection>('dashboard.summary');
  const data = query.data?.data[0];
  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">Command center</p><h1>Operational overview</h1><p>Server-authoritative status across missions, tasks, approvals, and alerts.</p></div><button className="button-secondary" onClick={() => query.refetch()} disabled={query.isFetching}>Refresh</button></header>
    {query.isLoading ? <div className="skeleton-panel" aria-label="Loading dashboard" /> : query.isError || !data ? <ErrorState onRetry={() => query.refetch()} /> : <><AlertBanner data={data} /><StatsGrid data={data} /><div className="dashboard-grid"><section className="panel"><div className="panel-heading"><div><p className="eyebrow">Recent activity</p><h2>Latest changes</h2></div></div><ActivityFeed items={data.activity} /></section><section className="panel"><div className="panel-heading"><div><p className="eyebrow">Next actions</p><h2>Operator queue</h2></div></div><div className="quick-actions"><Link to="/approvals">Review {data.pending_approvals} approvals</Link><Link to="/notifications">Acknowledge {data.unread_notifications} alerts</Link><Link to="/tasks">Inspect {data.blocked_tasks} blocked tasks</Link></div></section></div></>}
  </div>;
}
function ErrorState({ onRetry }: { onRetry: () => void }) { return <div className="empty-state" role="alert"><h2>Dashboard unavailable</h2><p>The server projection could not be loaded.</p><button className="button-primary" onClick={onRetry}>Retry</button></div>; }
