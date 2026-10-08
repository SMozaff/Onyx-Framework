import { Link } from 'react-router-dom';
import { StatusBadge } from '../../components/StatusBadge';
import { Freshness } from '../../components/Freshness';
import { ProjectionStatePanel } from '../../components/ProjectionStatePanel';
import { deriveProjectionState } from '../../components/ProjectionState';
import { useObserverQuery } from '../../hooks/useQuery';
import type { TaskSummary } from '../../types/query';
import { useI18n } from '../../i18n/I18nContext';

export function TasksPage() {
  const { t } = useI18n();
  const query = useObserverQuery<TaskSummary>('task.list');
  const state = deriveProjectionState(query);
  const tasks = query.data?.data ?? [];

  return (
    <div className="space-y-6">
      <header className="flex items-start justify-between">
        <div>
          <p className="text-xs uppercase tracking-wide text-slate-500">{t("common.readOnlyProjection")}</p>
          <h2 className="text-lg font-semibold text-slate-900">{t("tasks.title")}</h2>
          <p className="text-sm text-slate-600">{t("tasks.reviewOwnershipPriorityDue")}</p>
        </div>
        <Freshness state={state} />
      </header>

      {state.kind === 'loading' && <p className="text-sm text-slate-500">{t("common.loading")}</p>}
      {state.kind === 'unavailable' && <ProjectionStatePanel resource="Tasks" state={state} />}
      {state.kind === 'empty' && (
        <p className="text-sm text-slate-500">{t("tasks.noTasksVisible")}</p>
      )}
      {state.kind === 'stale' && <ProjectionStatePanel resource="Tasks" state={state} compact />}

      {tasks.length > 0 && (
        <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
          {tasks.map((task) => (
            <li key={task.id}>
              <Link
                to={`/task/${task.id}`}
                className="flex items-center justify-between px-4 py-3 hover:bg-slate-50"
              >
                <div>
                  <p className="font-medium text-slate-900">{task.title}</p>
                  <p className="text-xs text-slate-500">
                    {task.owner} · {new Date(task.due_at).toLocaleString()}
                  </p>
                </div>
                <StatusBadge status={task.status} />
              </Link>
            </li>
          ))}
        </ul>
      )}
      <footer className="text-xs text-slate-400">
        {query.data?.total_count ?? tasks.length} total · projection version{' '}
        {query.data?.freshness.projection_version ?? '—'}
      </footer>
    </div>
  );
}