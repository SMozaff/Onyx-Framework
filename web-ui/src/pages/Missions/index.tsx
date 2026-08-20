import { useEffect, useState } from 'react';
import { useOnyxQuery } from '../../hooks/useQuery';
import { deriveProjectionState, Freshness, ProjectionStatePanel } from '../../components/ProjectionState';
import type { MissionSummary } from '../../types/query';
import MissionList from './MissionList';
import MissionDetail from './MissionDetail';

export default function MissionsPage() {
  const query = useOnyxQuery<MissionSummary>('mission.list');
  const state = deriveProjectionState(query);
  const [selected, setSelected] = useState<MissionSummary | null>(null);
  useEffect(() => { if (!selected && query.data?.data[0]) setSelected(query.data.data[0]); }, [query.data, selected]);

  if (state.kind === 'loading') return <div className="page-stack"><div className="skeleton-list" aria-label="Loading missions" /></div>;
  if (state.kind === 'unavailable') return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">Read-only projection</p><h1>Missions</h1><p>Review purpose, ownership, lifecycle status, and temporal constraints.</p></div><Freshness state={state} /></header><ProjectionStatePanel resource="Missions" state={state} /></div>;

  const missions = query.data?.data ?? [];
  const total = query.data?.total_count ?? missions.length;
  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">Read-only projection</p><h1>Missions</h1><p>Review purpose, ownership, lifecycle status, and temporal constraints.</p></div><Freshness state={state} /></header>{state.kind === 'stale' && <ProjectionStatePanel resource="Missions" state={state} compact />}<div className="master-detail"><section className="panel"><div className="panel-heading"><h2>Mission portfolio</h2><span>{total} total</span></div>{state.kind === 'empty' ? <div className="empty-state"><h3>No missions</h3><p>No mission projections are currently available for this organization.</p></div> : <MissionList missions={missions} selectedId={selected?.id ?? null} onSelect={setSelected} />}</section>{selected ? <MissionDetail mission={selected} /> : <div className="detail-panel empty-state"><h2>{state.kind === 'empty' ? 'No mission selected' : 'Select a mission'}</h2></div>}</div></div>;
}
