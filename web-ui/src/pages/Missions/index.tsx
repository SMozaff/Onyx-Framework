import { useEffect, useState } from 'react';
import { useOnyxQuery } from '../../hooks/useQuery';
import type { MissionSummary } from '../../types/query';
import MissionList from './MissionList';
import MissionDetail from './MissionDetail';

export default function MissionsPage() {
  const query = useOnyxQuery<MissionSummary>('mission.list');
  const [selected, setSelected] = useState<MissionSummary | null>(null);
  useEffect(() => { if (!selected && query.data?.data[0]) setSelected(query.data.data[0]); }, [query.data, selected]);
  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">Read-only projection</p><h1>Missions</h1><p>Review purpose, ownership, lifecycle status, and temporal constraints.</p></div><Freshness value={query.data?.freshness.last_updated_at} stale={query.data?.freshness.is_stale} /></header><div className="master-detail"><section className="panel"><div className="panel-heading"><h2>Mission portfolio</h2><span>{query.data?.total_count ?? 0} total</span></div>{query.isLoading ? <div className="skeleton-list" /> : query.data?.data.length ? <MissionList missions={query.data.data} selectedId={selected?.id ?? null} onSelect={setSelected} /> : <div className="empty-state"><h3>No missions</h3><p>No mission projections are available.</p></div>}</section>{selected ? <MissionDetail mission={selected} /> : <div className="detail-panel empty-state"><h2>Select a mission</h2></div>}</div></div>;
}
function Freshness({ value, stale }: { value?: string; stale?: boolean }) { return <div className={`freshness ${stale ? 'freshness-stale' : ''}`}><span>{stale ? 'Stale projection' : 'Current projection'}</span><small>{value ? new Date(value).toLocaleString() : 'Not loaded'}</small></div>; }
