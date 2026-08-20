import { useState } from 'react';
import ApprovalDialog from '../../components/ApprovalDialog';
import { deriveProjectionState, Freshness, ProjectionStatePanel } from '../../components/ProjectionState';
import StatusBadge from '../../components/StatusBadge';
import { useApprovalDecision } from '../../hooks/useCommand';
import { useOnyxQuery } from '../../hooks/useQuery';
import type { ApprovalProjection } from '../../types/query';
import { normalizeError } from '../../utils/errorHandler';

export default function ApprovalsPage() {
  const query = useOnyxQuery<ApprovalProjection>('approval.list');
  const state = deriveProjectionState(query);
  const mutation = useApprovalDecision();
  const [dialog, setDialog] = useState<{ approval: ApprovalProjection; decision: 'approve' | 'reject' } | null>(null);
  const confirm = (reason: string) => {
    if (!dialog) return;
    mutation.mutate({ ...dialog, reason }, { onSuccess: () => setDialog(null) });
  };
  const approvals = query.data?.data ?? [];

  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">Authority workflow</p><h1>Approvals</h1><p>Decisions are pinned to the displayed target version and epochs.</p></div><Freshness state={state} /></header>
    {state.kind === 'loading' ? <div className="skeleton-list" aria-label="Loading approvals" /> : state.kind === 'unavailable' ? <ProjectionStatePanel resource="Approvals" state={state} /> : <>{state.kind === 'stale' && <ProjectionStatePanel resource="Approvals" state={state} compact />}<div className="card-list">{state.kind === 'empty' ? <section className="empty-state"><h2>No approvals</h2><p>No approval projections are currently available for this organization.</p></section> : approvals.map((approval) => <article className="approval-card" key={approval.id}><div className="approval-main"><div className="card-title-row"><h2>{approval.title}</h2><StatusBadge status={approval.status} /></div><p>{approval.description}</p><dl className="inline-details"><div><dt>Requested by</dt><dd>{approval.requested_by}</dd></div><div><dt>Target</dt><dd>{approval.target_type}</dd></div><div><dt>Version</dt><dd>{approval.version}</dd></div></dl></div>{approval.status === 'pending' && approval.web_action_permitted ? <div className="approval-actions"><button type="button" className="button-secondary" onClick={() => setDialog({ approval, decision: 'reject' })}>Reject</button><button type="button" className="button-primary" onClick={() => setDialog({ approval, decision: 'approve' })}>Approve</button></div> : approval.status === 'pending' ? <div className="restricted-action"><strong>Native client required</strong><span>Web-based decision is not permitted by policy.</span></div> : null}</article>)}</div></>}
    {dialog ? <ApprovalDialog {...dialog} busy={mutation.isPending} error={mutation.error ? normalizeError(mutation.error).message : null} onCancel={() => setDialog(null)} onConfirm={confirm} /> : null}
  </div>;
}
