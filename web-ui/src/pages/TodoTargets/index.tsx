import { useMemo, useState } from 'react';
import { useOnyxQuery } from '../../hooks/useQuery';
import { useAuthStore } from '../../stores/authStore';
import type { TargetListProjection, TodoListProjection } from '../../types/query';
import CreateListForm from './CreateListForm';
import ListCard from './ListCard';
import ListDetail from './ListDetail';

/**
 * Todo/Target lists share a master-detail workflow. The page now exposes a
 * focused escalated-to-you view alongside the general list: choosing an item
 * there opens the same action-capable detail panel instead of a read-only
 * escalation inbox.
 */
export default function TodoTargetsPage() {
  const [kind, setKind] = useState<'todo_list' | 'target_list'>('todo_list');
  const [view, setView] = useState<'all' | 'escalated'>('all');
  const user = useAuthStore((state) => state.user);

  const todoQuery = useOnyxQuery<TodoListProjection>('todo_list.list', {}, { enabled: kind === 'todo_list' });
  const targetQuery = useOnyxQuery<TargetListProjection>('target_list.list', {}, { enabled: kind === 'target_list' });

  const query = kind === 'todo_list' ? todoQuery : targetQuery;
  const lists = useMemo(() => {
    const all = query.data?.data ?? [];
    if (view === 'all' || !user) return all;
    return all.filter((list) => list.escalated_to === user.id);
  }, [query.data, user, view]);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = lists.find((list) => list.id === selectedId) ?? null;

  function selectKind(nextKind: 'todo_list' | 'target_list') {
    setKind(nextKind);
    setSelectedId(null);
  }

  function selectView(nextView: 'all' | 'escalated') {
    setView(nextView);
    setSelectedId(null);
  }

  return (
    <div className="page-stack">
      <header className="page-header">
        <div>
          <p className="eyebrow">Staff workflow</p>
          <h1>Todos &amp; Targets</h1>
          <p>Task lists and time-bound targets, verified by the owner's manager.</p>
        </div>
        <div style={{ display: 'grid', gap: 10, justifyItems: 'end' }}>
          <div style={{ display: 'flex', gap: 10 }} role="tablist" aria-label="List kind">
            <button
              type="button"
              role="tab"
              aria-selected={kind === 'todo_list'}
              className={kind === 'todo_list' ? 'button-primary' : 'button-secondary'}
              onClick={() => selectKind('todo_list')}
            >
              Todo lists
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={kind === 'target_list'}
              className={kind === 'target_list' ? 'button-primary' : 'button-secondary'}
              onClick={() => selectKind('target_list')}
            >
              Targets
            </button>
          </div>
          <label className="filter-label">
            Show
            <select value={view} onChange={(event) => selectView(event.target.value as 'all' | 'escalated')}>
              <option value="all">All {kind === 'todo_list' ? 'todo lists' : 'targets'}</option>
              <option value="escalated">Escalated to you</option>
            </select>
          </label>
        </div>
      </header>

      {user ? <CreateListForm kind={kind} ownerId={user.id} /> : null}

      <div className="master-detail">
        <section className="panel">
          <div className="panel-heading">
            <h2>{view === 'escalated' ? 'Escalated to you' : kind === 'todo_list' ? 'Todo lists' : 'Targets'}</h2>
            <span>{lists.length} shown</span>
          </div>
          {query.isLoading ? (
            <div className="skeleton-list" />
          ) : lists.length === 0 ? (
            <p style={{ color: 'var(--muted)', padding: '12px 4px' }}>
              {view === 'escalated'
                ? `No ${kind === 'todo_list' ? 'todo lists' : 'targets'} have been escalated to you.`
                : `None yet. ${kind === 'todo_list' ? 'Create a todo list' : 'Create a target'} above to get started.`}
            </p>
          ) : (
            <ListCard lists={lists} kind={kind} selectedId={selectedId} onSelect={setSelectedId} />
          )}
        </section>
        {selected ? (
          <ListDetail list={selected} kind={kind} />
        ) : (
          <div className="detail-panel empty-state">
            <h2>{view === 'escalated' ? 'Select an escalated item' : 'Select a list'}</h2>
          </div>
        )}
      </div>
    </div>
  );
}
