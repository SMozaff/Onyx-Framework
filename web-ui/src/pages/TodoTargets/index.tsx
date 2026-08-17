import { useMemo, useState } from 'react';
import { useOnyxQuery } from '../../hooks/useQuery';
import { useAuthStore } from '../../stores/authStore';
import type { TargetListProjection, TodoListProjection } from '../../types/query';
import CreateListForm from './CreateListForm';
import ListCard from './ListCard';
import ListDetail from './ListDetail';

/**
 * Todo/Target lists — list + detail, following `Approvals`/`Tasks`'s
 * established master-detail shape. Combines both aggregates in one
 * page (a `kind` toggle) since they share almost all UI logic per
 * design doc §4.0.2's confirmed shared verification structure — see
 * `todo_domain::aggregate`'s own header comment on why `TodoList`/
 * `TargetList` are separate Rust types with a shared state machine but
 * distinct content shape (items vs. a time window).
 *
 * This app (`web-ui`) is the correct home for this feature, not
 * `admin-shell` — design doc §4.0.1 confirms Todo/Target creation is
 * bidirectional (Staff or Manager), and `admin-shell`'s entire route
 * tree is gated on `is_admin` (see its `App.tsx`), which would wrongly
 * exclude every non-Admin user from a feature they're meant to use
 * directly.
 */
export default function TodoTargetsPage() {
  const [kind, setKind] = useState<'todo_list' | 'target_list'>('todo_list');
  const user = useAuthStore((state) => state.user);

  const todoQuery = useOnyxQuery<TodoListProjection>('todo_list.list', {}, { enabled: kind === 'todo_list' });
  const targetQuery = useOnyxQuery<TargetListProjection>('target_list.list', {}, { enabled: kind === 'target_list' });

  const query = kind === 'todo_list' ? todoQuery : targetQuery;
  const lists = useMemo(() => query.data?.data ?? [], [query.data]);

  const [selectedId, setSelectedId] = useState<string | null>(null);
  const selected = lists.find((l) => l.id === selectedId) ?? null;

  return (
    <div className="page-stack">
      <header className="page-header">
        <div>
          <p className="eyebrow">Staff workflow</p>
          <h1>Todos &amp; Targets</h1>
          <p>Task lists and time-bound targets, verified by the owner's manager.</p>
        </div>
        <div style={{ display: 'flex', gap: 10 }} role="tablist" aria-label="List kind">
          <button
            type="button"
            role="tab"
            aria-selected={kind === 'todo_list'}
            className={kind === 'todo_list' ? 'button-primary' : 'button-secondary'}
            onClick={() => {
              setKind('todo_list');
              setSelectedId(null);
            }}
          >
            Todo lists
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={kind === 'target_list'}
            className={kind === 'target_list' ? 'button-primary' : 'button-secondary'}
            onClick={() => {
              setKind('target_list');
              setSelectedId(null);
            }}
          >
            Targets
          </button>
        </div>
      </header>

      {user ? <CreateListForm kind={kind} ownerId={user.id} /> : null}

      <div className="master-detail">
        <section className="panel">
          <div className="panel-heading">
            <h2>{kind === 'todo_list' ? 'Todo lists' : 'Targets'}</h2>
            <span>{lists.length} shown</span>
          </div>
          {query.isLoading ? (
            <div className="skeleton-list" />
          ) : lists.length === 0 ? (
            <p style={{ color: 'var(--muted)', padding: '12px 4px' }}>
              None yet. {kind === 'todo_list' ? 'Create a todo list' : 'Create a target'} above to get started.
            </p>
          ) : (
            <ListCard lists={lists} kind={kind} selectedId={selectedId} onSelect={setSelectedId} />
          )}
        </section>
        {selected ? (
          <ListDetail list={selected} kind={kind} />
        ) : (
          <div className="detail-panel empty-state">
            <h2>Select a list</h2>
          </div>
        )}
      </div>
    </div>
  );
}
