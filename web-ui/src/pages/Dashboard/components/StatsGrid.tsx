import { useI18n } from '../../../i18n/I18nContext';
import type { DashboardProjection } from '../../../types/query';

const stats = [
  ['dashboard.activeMissions', 'active_missions'],
  ['dashboard.openTasks', 'tasks'],
  ['dashboard.blockedTasks', 'blocked_tasks'],
  ['dashboard.unreadAlerts', 'unread_notifications'],
  ['dashboard.pendingApprovals', 'pending_approvals'],
] as const;

export default function StatsGrid({ data }: { data: DashboardProjection }) {
  const { t } = useI18n();
  return <div className="stats-grid">{stats.map(([label, key]) => <article className="stat-card" key={key}><span>{t(label)}</span><strong>{data[key]}</strong></article>)}</div>;
}
