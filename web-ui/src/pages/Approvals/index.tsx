import { useI18n } from '../../i18n/I18nContext';
import { useState } from 'react';
import ApprovalDialog from '../../components/ApprovalDialog';
import { deriveProjectionState, Freshness, ProjectionStatePanel } from '../../components/ProjectionState';
import StatusBadge from '../../components/StatusBadge';
import { useApprovalDecision } from '../../hooks/useCommand';
import { useOnyxQuery } from '../../hooks/useQuery';
import type { ApprovalProjection } from '../../types/query';
import { normalizeError } from '../../utils/errorHandler';

export default function ApprovalsPage() {
  const { t } = useI18n();
  const query = useOnyxQuery<ApprovalProjection>('approval.list');
  const state = deriveProjectionState(query);
  const mutation = useApprovalDecision();
  const [dialog, setDialog] = useState<{ approval: ApprovalProjection; decision: 'approve' | 'reject' } | null>(null);
  const confirm = (reason: string) => {
    if (!dialog) return;
    mutation.mutate({ ...dialog, reason }, { onSuccess: () => setDialog(null) });
  };
  const approvals = query.data?.data ?? [];

  return <div className="page-stack"><header className="page-header"><div><p className="eyebrow">{t('approvals.authorityWorkflow')}</p><h1>{t('nav.approvals')}</h1><p>{t('approvals.decisionsPinned')}</p></div><Freshness state={state} /></header>
    {state.kind === 'loading' ? <div className="skeleton-list" aria-label={t("approvals.loading")} /> : state.kind === 'unavailable' ? <ProjectionStatePanel resource="Approvals" state={state} /> : <>{state.kind === 'stale' && <ProjectionStatePanel resource="Approvals" state={state} compact />}<div className="card-list">{state.kind === 'empty' ? <section className="empty-state"><h2>{t('approvals.noApprovals')}</h2><p>{t('approvals.noApprovalsBody')}</p></section> : approvals.map((approval) => <article className="approval-card" key={approval.id}><div className="approval-main"><div className="card-title-row"><h2>{approval.title}</h2><StatusBadge status={approval.status} /></div><p>{approval.description}</p><dl className="inline-details"><div><dt>{t('approvals.requestedBy')}</dt><dd>{approval.requested_by}</dd></div><div><dt>{t('approvals.target')}</dt><dd>{approval.target_type}</dd></div><div><dt>{t('common.version')}</dt><dd>{approval.version}</dd></div></dl></div>{approval.status === 'pending' && approval.web_action_permitted ? <div className="approval-actions"><button type="button" className="button-secondary" onClick={() => setDialog({ approval, decision: 'reject' })}>{t('common.reject')}</button><button type="button" className="button-primary" onClick={() => setDialog({ approval, decision: 'approve' })}>{t('common.approve')}</button></div> : approval.status === 'pending' ? <div className="restricted-action"><strong>{t('approvals.nativeClientRequired')}</strong><span>{t('approvals.webDecisionNotPermitted')}</span></div> : null}</article>)}</div></>}
    {dialog ? <ApprovalDialog {...dialog} busy={mutation.isPending} error={mutation.error ? normalizeError(mutation.error).message : null} onCancel={() => setDialog(null)} onConfirm={confirm} /> : null}
  </div>;
}
