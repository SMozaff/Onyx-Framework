import type { DashboardProjection } from '../../../types/query';

const stats = [
  ['Active missions', 'active_missions'], ['Open tasks', 'tasks'], ['Blocked tasks', 'blocked_tasks'],
  ['Unread alerts', 'unread_notifications'], ['Pending approvals', 'pending_approvals'],
] as const;

export default function StatsGrid({ data }: { data: DashboardProjection }) {
  return <div className="stats-grid">{stats.map(([label, key]) => <article className="stat-card" key={key}><span>{label}</span><strong>{data[key]}</strong></article>)}</div>;
}
