import StatusBadge from '../../components/StatusBadge';
import type { TargetListProjection, TodoListProjection } from '../../types/query';

type ListItem = TodoListProjection | TargetListProjection;

/**
 * The master list of Todo/Target rows. Matches
 * `Missions/MissionList.tsx`'s exact `data-list`/`data-row` shape (the
 * established working pattern this app's CSS actually supports) rather
 * than a new list structure.
 */
export default function ListCard({
  lists,
  kind,
  selectedId,
  onSelect,
}: {
  lists: ListItem[];
  kind: 'todo_list' | 'target_list';
  selectedId: string | null;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="data-list" role="list" aria-label={kind === 'todo_list' ? 'Todo lists' : 'Targets'}>
      {lists.map((list) => (
        <button
          type="button"
          role="listitem"
          key={list.id}
          className={`data-row ${selectedId === list.id ? 'data-row-selected' : ''}`}
          onClick={() => onSelect(list.id)}
        >
          <div>
            <strong>{describe(list, kind)}</strong>
            <span>{list.origin === 'ManagerAssigned' ? 'Assigned' : 'Self-authored'}</span>
          </div>
          <div className="row-meta">
            <StatusBadge status={list.status} />
          </div>
        </button>
      ))}
    </div>
  );
}

function describe(list: ListItem, kind: 'todo_list' | 'target_list'): string {
  if (kind === 'todo_list') {
    const todo = list as TodoListProjection;
    const count = todo.items?.length ?? 0;
    return `${count} item${count === 1 ? '' : 's'}`;
  }
  const target = list as TargetListProjection;
  return target.description || '(no description)';
}
