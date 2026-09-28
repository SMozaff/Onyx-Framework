import { Link } from 'react-router-dom';
import { useOnyxQuery } from '../../hooks/useQuery';
import { deriveProjectionState, Freshness, ProjectionStatePanel } from '../../components/ProjectionState';
import { useI18n } from '../../i18n/I18nContext';
import type { DashboardProjection } from '../../types/query';
import StatsGrid from './components/StatsGrid';
import AlertBanner from './components/AlertBanner';
import ActivityFeed from './components/ActivityFeed';

export default function DashboardPage() {
  const { t } = useI18n();
  const query = useOnyxQuery<DashboardProjection>('dashboard.summary');
  const state = deriveProjectionState(query);
  const data = query.data?.data[0];

  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">{t('dashboard.operationalOverview')}</p><h1>{t('nav.dashboard')}</h1><p>{t('common.readOnlyScope')}</p></div><div className="topbar-actions"><Freshness state={state} /><button type="button" className="button-secondary" onClick={() => void query.refetch()} disabled={query.isFetching}>{t('common.refresh')}</button></div></header>
    {state.kind === 'loading' ? <div className="skeleton-panel" aria-label={t('common.loading')} /> : state.kind === 'unavailable' ? <ProjectionStatePanel resource={t('nav.dashboard')} state={state} /> : !data ? <section className="empty-state"><h2>{t('common.noData')}</h2><p>{t('common.readOnlyScope')}</p></section> : <><>{state.kind === 'stale' && <ProjectionStatePanel resource={t('nav.dashboard')} state={state} compact />}</><AlertBanner data={data} /><StatsGrid data={data} /><div className="dashboard-grid"><section className="panel"><div className="panel-heading"><div><p className="eyebrow">{t('dashboard.recentActivity')}</p><h2>{t('dashboard.latestChanges')}</h2></div></div><ActivityFeed items={data.activity} /></section><section className="panel"><div className="panel-heading"><div><p className="eyebrow">{t('dashboard.operatorQueue')}</p><h2>{t('nav.missions')}</h2></div></div><div className="quick-actions"><Link to="/approvals">{t('approvals.reviewApproval')}</Link><Link to="/notifications">{t('notifications.acknowledge')}</Link><Link to="/tasks">{t('tasks.staffWorkflow')}</Link></div></section></div></>}
  </div>;
}
