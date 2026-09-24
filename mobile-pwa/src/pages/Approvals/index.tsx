import { StatusBadge } from '../../components/StatusBadge';
import { Freshness } from '../../components/Freshness';
import { ProjectionStatePanel } from '../../components/ProjectionStatePanel';
import { deriveProjectionState } from '../../components/ProjectionState';
import { useObserverQuery } from '../../hooks/useQuery';
import type { ApprovalProjection } from '../../types/query';

export function ApprovalsPage() {
  const query = useObserverQuery<ApprovalProjection>('approval.list');
  const state = deriveProjectionState(query);
  const approvals = query.data?.data ?? [];

  return (
    <div className="space-y-6">
      <header className="flex items-start justify-between">
        <div>
          <p className="text-xs uppercase tracking-wide text-slate-500">Authority workflow</p>
          <h2 className="text-lg font-semibold text-slate-900">Approvals</h2>
          <p className="text-sm text-slate-600">
            View-only observer scope: approving or rejecting requires the native client or a
            manager session.
          </p>
        </div>
        <Freshness state={state} />
      </header>

      {state.kind === 'loading' && <p className="text-sm text-slate-500">Loading…</p>}
      {state.kind === 'unavailable' && <ProjectionStatePanel resource="Approvals" state={state} />}
      {state.kind === 'empty' && (
        <p className="text-sm text-slate-500">No approval projections for your organization.</p>
      )}
      {state.kind === 'stale' && <ProjectionStatePanel resource="Approvals" state={state} compact />}

      {approvals.length > 0 && (
        <ul className="divide-y divide-slate-200 rounded-lg border border-slate-200 bg-white">
          {approvals.map((approval) => (
            <li key={approval.id} className="px-4 py-3">
              <div className="flex items-center justify-between gap-3">
                <p className="font-medium text-slate-900">{approval.title}</p>
                <StatusBadge status={approval.status} />
              </div>
              {approval.description ? (
                <p className="mt-1 text-sm text-slate-700">{approval.description}</p>
              ) : null}
              <dl className="mt-2 flex flex-wrap gap-x-6 gap-y-1 text-xs text-slate-500">
                <div>
                  <dt className="inline">Requested by </dt>
                  <dd className="inline font-medium text-slate-700">{approval.requested_by}</dd>
                </div>
                <div>
                  <dt className="inline">Target </dt>
                  <dd className="inline font-medium text-slate-700">{approval.target_type}</dd>
                </div>
                <div>
                  <dt className="inline">Version </dt>
                  <dd className="inline font-medium text-slate-700">{approval.version}</dd>
                </div>
              </dl>
              {approval.status === 'pending' ? (
                <p className="mt-2 rounded bg-amber-50 px-3 py-2 text-xs text-amber-800">
                  {approval.web_action_permitted
                    ? 'This approval is decisionable in the web client — but the observer client is decision-free by contract.'
                    : 'Native client required to decide this approval.'}
                </p>
              ) : approval.decided_at ? (
                <p className="mt-2 text-xs text-slate-500">
                  Decided {new Date(approval.decided_at).toLocaleString()}
                  {approval.decision_reason ? ` · ${approval.decision_reason}` : ''}
                </p>
              ) : null}
            </li>
          ))}
        </ul>
      )}
      <footer className="text-xs text-slate-400">
        projection version {query.data?.freshness.projection_version ?? '—'}
      </footer>
    </div>
  );
}