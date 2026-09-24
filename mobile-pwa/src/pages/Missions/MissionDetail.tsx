import { Link, Navigate, useParams } from 'react-router-dom';
import { StatusBadge } from '../../components/StatusBadge';
import { useObserverQuery } from '../../hooks/useQuery';
import type { MissionSummary, TimelineEntry } from '../../types/query';

export function MissionDetailPage() {
  const { id } = useParams<{ id: string }>();
  const mission = useObserverQuery<MissionSummary>('mission.detail', { id: id ?? '' });
  const timeline = useObserverQuery<TimelineEntry>('timeline.list', { subject_id: id ?? '' });

  if (!id) return <Navigate to="/missions" replace />;
  if (mission.isPending) return <p className="text-sm text-slate-500">Loading…</p>;
  if (mission.isError || !mission.data?.data[0]) {
    return (
      <Link className="text-sm text-slate-500 underline underline-offset-4" to="/missions">
        Mission unavailable — back to missions
      </Link>
    );
  }

  const { name, summary, status, owner, priority, progress, version } = mission.data.data[0];
  const entries = timeline.data?.data ?? [];

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <Link className="text-sm text-slate-500 underline underline-offset-4" to="/missions">
          ← All missions
        </Link>
        <StatusBadge status={status} />
      </div>

      <header>
        <p className="text-xs uppercase tracking-wide text-slate-500">Mission</p>
        <h2 className="text-xl font-semibold text-slate-900">{name}</h2>
        {summary ? <p className="mt-1 text-sm text-slate-700">{summary}</p> : null}
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
          <dt className="text-xs uppercase tracking-wide text-slate-500">Progress</dt>
          <dd className="text-sm font-medium text-slate-900">{progress}%</dd>
        </div>
        <div>
          <dt className="text-xs uppercase tracking-wide text-slate-500">Version</dt>
          <dd className="text-sm font-medium text-slate-900">{version}</dd>
        </div>
      </dl>

      <div className="h-2 w-full rounded bg-slate-200" aria-label={`${progress}% complete`}>
        <div className="h-2 rounded bg-slate-900" style={{ width: `${progress}%` }} />
      </div>

      <section aria-label="Timeline">
        <h3 className="mb-3 text-base font-semibold text-slate-900">Timeline</h3>
        {timeline.isPending ? (
          <p className="text-sm text-slate-500">Loading…</p>
        ) : entries.length === 0 ? (
          <p className="text-sm text-slate-500">No timeline entries in this projection.</p>
        ) : (
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {entries.map((item) => (
              <li key={item.id} className="flex items-center justify-between px-4 py-3">
                <div>
                  <p className="font-medium text-slate-900">{item.label}</p>
                  <p className="text-xs text-slate-500">{new Date(item.at).toLocaleString()}</p>
                </div>
                <StatusBadge status={item.status} />
              </li>
            ))}
          </ul>
        )}
      </section>

      <p className="rounded-lg bg-slate-100 px-3 py-2 text-xs text-slate-600">
        This is a read-only projection. Mission changes require a native client with the
        appropriate authority.
      </p>
    </div>
  );
}