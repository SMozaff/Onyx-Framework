import { useEffect, useMemo, useState } from 'react';
import { useOnyxQuery } from '../../hooks/useQuery';
import { deriveProjectionState, Freshness, ProjectionStatePanel } from '../../components/ProjectionState';
import type { TaskSummary } from '../../types/query';
import TaskList from './TaskList';
import TaskDetail from './TaskDetail';

export default function TasksPage() {
  const query = useOnyxQuery<TaskSummary>('task.list');
  const state = deriveProjectionState(query);
  const [selected, setSelected] = useState<TaskSummary | null>(null);
  const [status, setStatus] = useState('all');
  const filtered = useMemo(() => query.data?.data.filter((task) => status === 'all' || task.status === status) ?? [], [query.data, status]);
  useEffect(() => { if (!selected && filtered[0]) setSelected(filtered[0]); }, [filtered, selected]);

  const header = <header className="page-header"><div><p className="eyebrow">Execution visibility</p><h1>Tasks</h1><p>Inspect operational work without introducing client-side lifecycle logic.</p></div><div className="topbar-actions"><Freshness state={state} /><label className="filter-label">Status<select value={status} onChange={(e) => { setStatus(e.target.value); setSelected(null); }} disabled={state.kind === 'unavailable' || state.kind === 'loading'}><option value="all">All</option><option value="active">Active</option><option value="blocked">Blocked</option><option value="paused">Paused</option></select></label></div></header>;

  if (state.kind === 'loading') return <div className="page-stack">{header}<div className="skeleton-list" aria-label="Loading tasks" /></div>;
  if (state.kind === 'unavailable') return <div className="page-stack">{header}<ProjectionStatePanel resource="Tasks" state={state} /></div>;

  return <div className="page-stack">{header}{state.kind === 'stale' && <ProjectionStatePanel resource="Tasks" state={state} compact />}<div className="master-detail"><section className="panel"><div className="panel-heading"><h2>Work queue</h2><span>{filtered.length} shown</span></div>{filtered.length ? <TaskList tasks={filtered} selectedId={selected?.id ?? null} onSelect={setSelected} /> : <div className="empty-state"><h3>No tasks match this view</h3><p>{state.kind === 'empty' ? 'No task projections are currently available for this organization.' : 'Choose another status to review available work.'}</p></div>}</section>{selected ? <TaskDetail task={selected} /> : <div className="detail-panel empty-state"><h2>{filtered.length ? 'Select a task' : 'No task selected'}</h2></div>}</div></div>;
}
