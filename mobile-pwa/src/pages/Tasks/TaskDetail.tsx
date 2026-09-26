import { Link, Navigate, useParams } from 'react-router-dom';
import { StatusBadge } from '../../components/StatusBadge';
import { useObserverQuery } from '../../hooks/useQuery';
import type { TaskSummary } from '../../types/query';

export function TaskDetailPage() {
  const { id } = useParams<{ id: string }>();
  const task = useObserverQuery<TaskSummary>('task.detail', { id: id ?? '' });

  if (!id) return <Navigate to="/tasks" replace />;
  if (task.isPending) return <p className="text-sm text-slate-500">Loading…</p>;
  if (task.isError || !task.data?.data[0]) {
    return (
      <Link className="text-sm text-slate-500 underline underline-offset-4" to="/tasks">
        Task unavailable — back to tasks
      </Link>
    );
  }

  const { title, status, owner, priority, due_at, version, mission_id, updated_at } = task.data.data[0];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <Link className="text-sm text-slate-500 underline underline-offset-4" to="/tasks">
          ← All tasks
        </Link>
        <StatusBadge status={status} />
      </div>

      <header>
        <p className="text-xs uppercase tracking-wide text-slate-500">Task</p>
        <h2 className="text-xl font-semibold text-slate-900">{title}</h2>
      </header>

      <dl className="grid grid-cols-2 gap-3 rounded-lg border border-slate-200 bg-white p-4 sm:grid-cols-4">
        <div>
          <dt className="text-xs uppercase tracking-wide text-slate-500">Owner</dt>
          <dd className="text-sm font-medium text-slate-900">{owner}</dd>
        </div>
        <div>
          <dt className="text-xs uppercase tracking-wide text-slate-500">Priority</dt>
          <dd className="text-sm font-medium text-slate-900">{priority}</dd>
        </div>
        <div>
          <dt className="text-xs uppercase tracking-wide text-slate-500">Due</dt>
          <dd className="text-sm font-medium text-slate-900">{new Date(due_at).toLocaleString()}</dd>
        </div>
        <div>
          <dt className="text-xs uppercase tracking-wide text-slate-500">Version</dt>
          <dd className="text-sm font-medium text-slate-900">{version}</dd>
        </div>
        <div className="col-span-2">
          <dt className="text-xs uppercase tracking-wide text-slate-500">Mission</dt>
          <dd className="break-all text-sm font-medium text-slate-900">{mission_id}</dd>
        </div>
        <div className="col-span-2">
          <dt className="text-xs uppercase tracking-wide text-slate-500">Updated</dt>
          <dd className="text-sm font-medium text-slate-900">
            {new Date(updated_at).toLocaleString()}
          </dd>
        </div>
      </dl>

      <p className="rounded-lg bg-slate-100 px-3 py-2 text-xs text-slate-600">
        This is a read-only projection. Task lifecycle and ownership changes require a native
        client with the appropriate authority.
      </p>
    </div>
  );
}