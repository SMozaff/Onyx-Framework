import { useState } from 'react';
import type { ApprovalProjection } from '../../types/query';

interface Props {
  approval: ApprovalProjection;
  decision: 'approve' | 'reject';
  busy: boolean;
  onCancel: () => void;
  onConfirm: (reason: string) => void;
}

export default function ApprovalDialog({ approval, decision, busy, onCancel, onConfirm }: Props) {
  const [reason, setReason] = useState('');
  const rejectNeedsReason = decision === 'reject';
  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onCancel()}>
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="approval-dialog-title">
        <h2 id="approval-dialog-title">{decision === 'approve' ? 'Approve' : 'Reject'} request</h2>
        <p>{approval.title}</p>
        <label htmlFor="decision-reason">Decision reason {rejectNeedsReason ? '(required)' : '(optional)'}</label>
        <textarea id="decision-reason" value={reason} onChange={(event) => setReason(event.target.value)} rows={4} autoFocus />
        <div className="dialog-actions">
          <button className="button-secondary" type="button" onClick={onCancel} disabled={busy}>Cancel</button>
          <button className={decision === 'approve' ? 'button-primary' : 'button-danger'} type="button" onClick={() => onConfirm(reason)} disabled={busy || (rejectNeedsReason && !reason.trim())}>
            {busy ? 'Submitting…' : decision === 'approve' ? 'Confirm approval' : 'Confirm rejection'}
          </button>
        </div>
      </section>
    </div>
  );
}
