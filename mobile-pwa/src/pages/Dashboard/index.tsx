import { observerApi } from '../../api/onyx';
import { observerQueryKeys, useObserverQuery } from '../../hooks/useQuery';
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
  const dashboardQuery = useObserverQuery<DashboardProjection>(
    observerQueryKeys.dashboard(),
    async () => observerApi.query<DashboardProjection>('dashboard.summary'),
  );

  const missionsQuery = useObserverQuery<MissionSummary>(
    observerQueryKeys.missions(),
    async () => observerApi.query<MissionSummary>('mission.list', {}, { limit: 5 }),
  );

  const approvalsQuery = useObserverQuery<ApprovalProjection>(
    observerQueryKeys.approvals(),
    async () =>
      observerApi.query<ApprovalProjection>('approval.list', { status: 'pending' }, { limit: 3 }),
  );

  const dashboard = dashboardQuery.data?.data?.[0];
  const missions = missionsQuery.data?.data ?? [];
  const approvals = approvalsQuery.data?.data ?? [];

  return (
    <div className="space-y-6">
      <section>
        <h2 className="mb-3 text-lg font-semibold text-slate-900">Overview</h2>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <StatCard label="Missions" value={dashboard?.missions ?? missions.length} />
          <StatCard label="Active" value={dashboard?.active_missions ?? 0} />
          <StatCard label="Unread" value={dashboard?.unread_notifications ?? 0} />
          <StatCard label="Approvals" value={dashboard?.pending_approvals ?? approvals.length} />
        </div>
      </section>

      <section>
        <h2 className="mb-3 text-lg font-semibold text-slate-900">Recent missions</h2>
        {missionsQuery.isPending ? (
          <p className="text-sm text-slate-500">Loading…</p>
        ) : missions.length === 0 ? (
          <p className="text-sm text-slate-500">No missions visible to your account.</p>
        ) : (
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {missions.map((mission) => (
              <li key={mission.id} className="flex items-center justify-between px-4 py-3">
                <div>
                  <p className="font-medium text-slate-900">{mission.name}</p>
                  <p className="text-xs text-slate-500">{mission.status}</p>
                </div>
                <span className="text-xs text-slate-500">{mission.progress}%</span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section>
        <h2 className="mb-3 text-lg font-semibold text-slate-900">Pending approvals</h2>
        {approvalsQuery.isPending ? (
          <p className="text-sm text-slate-500">Loading…</p>
        ) : approvals.length === 0 ? (
          <p className="text-sm text-slate-500">No pending approvals.</p>
        ) : (
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {approvals.map((approval) => (
              <li key={approval.id} className="flex items-center justify-between px-4 py-3">
                <div>
                  <p className="font-medium text-slate-900">{approval.title}</p>
                  <p className="text-xs text-slate-500">Requested by {approval.requested_by}</p>
                </div>
                <span className="text-xs text-slate-500">
                  {approval.web_action_permitted ? 'decisionable' : 'read-only'}
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}