import { Link } from 'react-router-dom';
import { StatusBadge } from '../../components/StatusBadge';
import { Freshness } from '../../components/Freshness';
import { ProjectionStatePanel } from '../../components/ProjectionStatePanel';
import { deriveProjectionState } from '../../components/ProjectionState';
import { useObserverQuery } from '../../hooks/useQuery';
import type { MissionSummary } from '../../types/query';
import { useI18n } from '../../i18n/I18nContext';

export function MissionsPage() {
  const { t } = useI18n();
  const query = useObserverQuery<MissionSummary>('mission.list');
  const state = deriveProjectionState(query);
  const missions = query.data?.data ?? [];

  return (
    <div className="space-y-6">
      <header className="flex items-start justify-between">
        <div>
          <p className="text-xs uppercase tracking-wide text-slate-500">{t("common.readOnlyProjection")}</p>
          <h2 className="text-lg font-semibold text-slate-900">{t("missions.title")}</h2>
          <p className="text-sm text-slate-600">
            {t("missions.reviewPurposeOwnership")}
          </p>
        </div>
        <Freshness state={state} />
      </header>

      {state.kind === 'loading' && <p className="text-sm text-slate-500">{t("common.loading")}</p>}
      {state.kind === 'unavailable' && <ProjectionStatePanel resource="Missions" state={state} />}
      {state.kind === 'empty' && (
        <p className="text-sm text-slate-500">{t("dashboard.noMissionsVisible")}</p>
      )}
      {state.kind === 'stale' && <ProjectionStatePanel resource="Missions" state={state} compact />}

      {missions.length > 0 && (
        <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
          {missions.map((mission) => (
            <li key={mission.id}>
              <Link
                to={`/mission/${mission.id}`}
                className="flex items-center justify-between px-4 py-3 hover:bg-slate-50"
              >
                <div>
                  <p className="font-medium text-slate-900">{mission.name}</p>
                  <p className="text-xs text-slate-500">{mission.owner}</p>
                </div>
                <div className="flex items-center gap-3">
                  <span className="text-xs text-slate-500">{mission.progress}%</span>
                  <StatusBadge status={mission.status} />
                </div>
              </Link>
            </li>
          ))}
        </ul>
      )}
      <footer className="text-xs text-slate-400">
        {query.data?.total_count ?? missions.length} total · projection version{' '}
        {query.data?.freshness.projection_version ?? '—'}
      </footer>
    </div>
  );
}