import { useObserverQuery } from '../../hooks/useQuery';
import type {
  DashboardProjection,
  MissionSummary,
  ApprovalProjection,
} from '../../types/query';

function StatCard({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-lg border border-slate-200 bg-white p-4">
      <p className="text-xs uppercase tracking-wide text-slate-500">{label}</p>
      <p className="mt-1 text-2xl font-bold text-slate-900">{value}</p>
    </div>
  );
}

export function DashboardPage() {
  const dashboard = useObserverQuery<DashboardProjection>('dashboard.summary');
  const missions = useObserverQuery<MissionSummary>('mission.list', {}, { select(data) { return { ...data, data: data.data.slice(0, 5) }; } });
  const approvals = useObserverQuery<ApprovalProjection>('approval.list', { status: 'pending' }, { select(data) { return { ...data, data: data.data.slice(0, 3) }; } });

  const summary = dashboard.data?.data?.[0];
  const missionList = missions.data?.data ?? [];
  const approvalList = approvals.data?.data ?? [];
  const activity = summary?.activity ?? [];

  return (
    <div className="space-y-6">
      <header>
        <p className="text-xs uppercase tracking-wide text-slate-500">Read-only projection</p>
        <h2 className="text-lg font-semibold text-slate-900">Dashboard</h2>
      </header>

      <section aria-label="Overview">
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <StatCard label="Missions" value={summary?.missions ?? missionList.length} />
          <StatCard label="Active" value={summary?.active_missions ?? 0} />
          <StatCard label="Unread" value={summary?.unread_notifications ?? 0} />
          <StatCard label="Approvals" value={summary?.pending_approvals ?? approvalList.length} />
        </div>
      </section>

      <section aria-label="Recent missions">
        <h3 className="mb-3 text-base font-semibold text-slate-900">Recent missions</h3>
        {missions.isPending ? (
          <p className="text-sm text-slate-500">Loading…</p>
        ) : missionList.length === 0 ? (
          <p className="text-sm text-slate-500">No missions visible to your account.</p>
        ) : (
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {missionList.map((mission) => (
              <li key={mission.id} className="flex items-center justify-between px-4 py-3">
                <div>
                  <p className="font-medium text-slate-900">{mission.name}</p>
                  <p className="text-xs text-slate-500">{mission.owner}</p>
                </div>
                <span className="text-xs text-slate-500">{mission.progress}%</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section aria-label="Recent activity">
        <h3 className="mb-3 text-base font-semibold text-slate-900">Recent activity</h3>
        {activity.length === 0 ? (
          <p className="text-sm text-slate-500">No activity recorded yet.</p>
        ) : (
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {activity.map((item) => (
              <li key={`${item.type}-${item.id}`} className="flex items-center justify-between px-4 py-3">
                <div>
                  <p className="font-medium text-slate-900">{item.title}</p>
                  <p className="text-xs text-slate-500">{item.type}</p>
                </div>
                <span className="text-xs text-slate-500">
                  {new Date(item.updated_at).toLocaleString()}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      {approvalList.length > 0 ? (
        <section aria-label="Pending approvals">
          <h3 className="mb-3 text-base font-semibold text-slate-900">Pending approvals</h3>
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {approvalList.map((approval) => (
              <li key={approval.id} className="px-4 py-3">
                <p className="font-medium text-slate-900">{approval.title}</p>
                <p className="text-xs text-slate-500">Requested by {approval.requested_by}</p>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}