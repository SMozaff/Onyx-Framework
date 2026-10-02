import { useObserverQuery } from '../../hooks/useQuery';
import { useI18n } from '../../i18n/I18nContext';
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
  const { t } = useI18n();
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
        <p className="text-xs uppercase tracking-wide text-slate-500">{t("common.readOnlyProjection")}</p>
        <h2 className="text-lg font-semibold text-slate-900">{t("dashboard.title")}</h2>
      </header>

      <section aria-label={t("nav.overview")}>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <StatCard label={t("nav.missions")} value={summary?.missions ?? missionList.length} />
          <StatCard label={t("status.active")} value={summary?.active_missions ?? 0} />
          <StatCard label={t("dashboard.unreadNotifications")} value={summary?.unread_notifications ?? 0} />
          <StatCard label={t("nav.approvals")} value={summary?.pending_approvals ?? approvalList.length} />
        </div>
      </section>

      <section aria-label={t("dashboard.recentMissions")}>
        <h3 className="mb-3 text-base font-semibold text-slate-900"{t("dashboard.recentMissions")}</h3>
        {missions.isPending ? (
          <p className="text-sm text-slate-500"{t("common.loading")}</p>
        ) : missionList.length === 0 ? (
          <p className="text-sm text-slate-500"{t("dashboard.noMissionsVisible")}</p>
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

      <section aria-label={t("dashboard.recentActivity")}>
        <h3 className="mb-3 text-base font-semibold text-slate-900"{t("dashboard.recentActivity")}</h3>
        {activity.length === 0 ? (
          <p className="text-sm text-slate-500"{t("dashboard.noActivity")}</p>
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
        <section aria-label={t("dashboard.pendingApprovals")}>
          <h3 className="mb-3 text-base font-semibold text-slate-900"{t("dashboard.pendingApprovals")}</h3>
          <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
            {approvalList.map((approval) => (
              <li key={approval.id} className="px-4 py-3">
                <p className="font-medium text-slate-900">{approval.title}</p>
                <p className="text-xs text-slate-500">{t("approvals.requestedBy")} {approval.requested_by}</p>
              </li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}