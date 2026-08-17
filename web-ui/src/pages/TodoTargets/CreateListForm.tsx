import { useState } from 'react';
import UserPicker from '../../components/UserPicker';
import { useCreateTargetList, useCreateTodoList } from '../../hooks/useCommand';
import type { ListOrigin } from '../../types/query';

/**
 * Creation form for a `TodoList` or `TargetList`. The picker-backed
 * assignment choice retains self-authored creation while allowing a Manager
 * to create work for another active colleague using the domain's explicit
 * `ManagerAssigned` origin.
 */
export default function CreateListForm({ kind, ownerId }: { kind: 'todo_list' | 'target_list'; ownerId: string }) {
  const createTodo = useCreateTodoList();
  const createTarget = useCreateTargetList();

  const [itemText, setItemText] = useState('');
  const [description, setDescription] = useState('');
  const [startAt, setStartAt] = useState('');
  const [endAt, setEndAt] = useState('');
  const [assignmentMode, setAssignmentMode] = useState<'self' | 'assign'>('self');
  const [assigneeId, setAssigneeId] = useState('');

  const assignedToSomeoneElse = assignmentMode === 'assign';
  const selectedOwner = assignedToSomeoneElse ? assigneeId : ownerId;
  const origin: ListOrigin = assignedToSomeoneElse ? 'ManagerAssigned' : 'StaffAuthored';
  const ownerSelected = Boolean(selectedOwner);

  function createTodoList() {
    if (itemText.trim().length === 0 || !ownerSelected) return;
    createTodo.mutate(
      { owner: selectedOwner, origin, items: [{ description: itemText.trim() }] },
      { onSuccess: () => setItemText('') },
    );
  }

  function createTargetList() {
    if (description.trim().length === 0 || !startAt || !endAt || !ownerSelected) return;
    const startMs = new Date(startAt).getTime();
    const endMs = new Date(endAt).getTime();
    if (Number.isNaN(startMs) || Number.isNaN(endMs) || endMs <= startMs) return;
    createTarget.mutate(
      { owner: selectedOwner, origin, description: description.trim(), time_window: { start_at_ms: startMs, end_at_ms: endMs } },
      {
        onSuccess: () => {
          setDescription('');
          setStartAt('');
          setEndAt('');
        },
      },
    );
  }

  const assignmentControl = (
    <fieldset style={{ display: 'grid', gap: 8, border: 0, padding: 0, margin: 0 }}>
      <legend className="muted" style={{ fontSize: 13 }}>
        Who is this for?
      </legend>
      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        <label>
          <input
            type="radio"
            name={`assignment-${kind}`}
            checked={!assignedToSomeoneElse}
            onChange={() => setAssignmentMode('self')}
          />{' '}
          For myself
        </label>
        <label>
          <input
            type="radio"
            name={`assignment-${kind}`}
            checked={assignedToSomeoneElse}
            onChange={() => setAssignmentMode('assign')}
          />{' '}
          Assign to someone else
        </label>
      </div>
      {assignedToSomeoneElse ? (
        <UserPicker
          label="Assignee"
          value={assigneeId}
          onChange={setAssigneeId}
          placeholder="Choose the person who will own this work"
        />
      ) : null}
    </fieldset>
  );

  if (kind === 'todo_list') {
    return (
      <section className="panel">
        <div className="panel-heading">
          <h2>Create a todo list</h2>
        </div>
        <p className="muted" style={{ marginTop: 0 }}>
          Starts with one item — add more from the detail view once created.
        </p>
        <div style={{ display: 'grid', gap: 10 }}>
          {assignmentControl}
          <div style={{ display: 'flex', gap: 10 }}>
            <input
              value={itemText}
              onChange={(event) => setItemText(event.target.value)}
              placeholder="First item, e.g. Write the weekly report"
            />
            <button
              type="button"
              className="button-primary"
              onClick={createTodoList}
              disabled={createTodo.isPending || itemText.trim().length === 0 || !ownerSelected}
            >
              {createTodo.isPending ? 'Creating…' : 'Create'}
            </button>
          </div>
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
        {assignmentControl}
        <input
          value={description}
          onChange={(event) => setDescription(event.target.value)}
          placeholder="What does hitting this target mean?"
        />
        <div style={{ display: 'flex', gap: 10 }}>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Window starts
            </span>
            <input type="datetime-local" value={startAt} onChange={(event) => setStartAt(event.target.value)} />
          </label>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Window ends
            </span>
            <input type="datetime-local" value={endAt} onChange={(event) => setEndAt(event.target.value)} />
          </label>
        </div>
        <button
          type="button"
          className="button-primary"
          onClick={createTargetList}
          disabled={createTarget.isPending || description.trim().length === 0 || !startAt || !endAt || !ownerSelected}
        >
          {createTarget.isPending ? 'Creating…' : 'Create'}
        </button>
      </div>
    </section>
  );
}
