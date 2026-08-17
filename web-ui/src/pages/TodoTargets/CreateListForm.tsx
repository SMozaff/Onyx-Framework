import { useState } from 'react';
import { useCreateTargetList, useCreateTodoList } from '../../hooks/useCommand';
import type { ListOrigin } from '../../types/query';

/**
 * Creation form for a `TodoList` or `TargetList`. Design doc §4.0.1
 * confirms bidirectional creation (Staff or Manager) — this form
 * always sets `owner` to the current user and `origin` to
 * `StaffAuthored`, since assigning a list to a *different* owner
 * (Manager-assigned) requires knowing that other person's user id,
 * which this UI has no lookup for yet. Self-authored creation is the
 * complete, correct first slice; Manager-assignment is a natural
 * follow-up once a user picker exists, not a compromise made here.
 */
export default function CreateListForm({ kind, ownerId }: { kind: 'todo_list' | 'target_list'; ownerId: string }) {
  const createTodo = useCreateTodoList();
  const createTarget = useCreateTargetList();

  const [itemText, setItemText] = useState('');
  const [description, setDescription] = useState('');
  const [startAt, setStartAt] = useState('');
  const [endAt, setEndAt] = useState('');

  const origin: ListOrigin = 'StaffAuthored';

  function createTodoList() {
    if (itemText.trim().length === 0) return;
    createTodo.mutate(
      { owner: ownerId, origin, items: [{ description: itemText.trim() }] },
      { onSuccess: () => setItemText('') },
    );
  }

  function createTargetList() {
    if (description.trim().length === 0 || !startAt || !endAt) return;
    const startMs = new Date(startAt).getTime();
    const endMs = new Date(endAt).getTime();
    if (Number.isNaN(startMs) || Number.isNaN(endMs) || endMs <= startMs) return;
    createTarget.mutate(
      { owner: ownerId, origin, description: description.trim(), time_window: { start_at_ms: startMs, end_at_ms: endMs } },
      {
        onSuccess: () => {
          setDescription('');
          setStartAt('');
          setEndAt('');
        },
      },
    );
  }

  if (kind === 'todo_list') {
    return (
      <section className="panel">
        <div className="panel-heading">
          <h2>Create a todo list</h2>
        </div>
        <p className="muted" style={{ marginTop: 0 }}>
          Starts with one item — add more from the detail view once created.
        </p>
        <div style={{ display: 'flex', gap: 10 }}>
          <input
            value={itemText}
            onChange={(e) => setItemText(e.target.value)}
            placeholder="First item, e.g. Write the weekly report"
          />
          <button
            type="button"
            className="button-primary"
            onClick={createTodoList}
            disabled={createTodo.isPending || itemText.trim().length === 0}
          >
            {createTodo.isPending ? 'Creating…' : 'Create'}
          </button>
        </div>
      </section>
    );
  }

  return (
    <section className="panel">
      <div className="panel-heading">
        <h2>Create a target</h2>
      </div>
      <div style={{ display: 'grid', gap: 10 }}>
        <input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="What does hitting this target mean?" />
        <div style={{ display: 'flex', gap: 10 }}>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Window starts
            </span>
            <input type="datetime-local" value={startAt} onChange={(e) => setStartAt(e.target.value)} />
          </label>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Window ends
            </span>
            <input type="datetime-local" value={endAt} onChange={(e) => setEndAt(e.target.value)} />
          </label>
        </div>
        <button
          type="button"
          className="button-primary"
          onClick={createTargetList}
          disabled={createTarget.isPending || description.trim().length === 0 || !startAt || !endAt}
        >
          {createTarget.isPending ? 'Creating…' : 'Create'}
        </button>
      </div>
    </section>
  );
}
