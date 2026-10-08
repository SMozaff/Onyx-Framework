import { useEffect, useState } from 'react';
import { useOnyxQuery } from '../../hooks/useQuery';
import { deriveProjectionState, Freshness, ProjectionStatePanel } from '../../components/ProjectionState';
import { useI18n } from '../../i18n/I18nContext';
import type { MissionSummary } from '../../types/query';
import MissionList from './MissionList';
import MissionDetail from './MissionDetail';

export default function MissionsPage() {
  const { t } = useI18n();
  const query = useOnyxQuery<MissionSummary>('mission.list');
  const state = deriveProjectionState(query);
  const [selected, setSelected] = useState<MissionSummary | null>(null);
  useEffect(() => { if (!selected && query.data?.data[0]) setSelected(query.data.data[0]); }, [query.data, selected]);

  if (state.kind === 'loading') return <div className="page-stack"><div className="skeleton-list" aria-label={t('common.loading')} /></div>;
  if (state.kind === 'unavailable') return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">{t('missions.singular')}</p><h1>{t('nav.missions')}</h1><p>{t('missions.portfolio')}</p></div><Freshness state={state} /></header><ProjectionStatePanel resource={t('nav.missions')} state={state} /></div>;

  const missions = query.data?.data ?? [];
  const total = query.data?.total_count ?? missions.length;
  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">{t('missions.singular')}</p><h1>{t('nav.missions')}</h1><p>{t('missions.portfolio')}</p></div><Freshness state={state} /></header>{state.kind === 'stale' && <ProjectionStatePanel resource={t('nav.missions')} state={state} compact />}<div className="master-detail"><section className="panel"><div className="panel-heading"><h2>{t('missions.portfolio')}</h2><span>{total} {t('common.total_items')}</span></div>{state.kind === 'empty' ? <div className="empty-state"><h3>{t('missions.noMissions')}</h3><p>{t('missions.noMissionsBody')}</p></div> : <MissionList missions={missions} selectedId={selected?.id ?? null} onSelect={setSelected} />}</section>{selected ? <MissionDetail mission={selected} /> : <div className="detail-panel empty-state"><h2>{state.kind === 'empty' ? t('missions.noMissions') : t('nav.missions')}</h2></div>}</div></div>;
}
