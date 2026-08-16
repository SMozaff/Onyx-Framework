import { useState } from 'react';
import type { VerificationOutcome } from '../../types/query';

interface Props {
  decision: 'verify' | 'reject' | 'escalate';
  busy: boolean;
  onCancel: () => void;
  onConfirm: (payload: { outcome?: VerificationOutcome; comment?: string; reason?: string }) => void;
}

/**
 * Confirms a verify/reject/escalate decision. Follows
 * `ApprovalDialog`'s exact structure (backdrop, dialog role, cancel +
 * confirm actions) but branches on `decision`:
 * - `verify`: outcome radio (Flawless/WithDeficiencies) plus an
 *   always-optional comment — design doc §4.0.1.1, resolved
 *   2026-08-16: "It's not like a requirement. More like a checkbox." A
 *   checkbox gates the comment field's visibility, matching that
 *   description directly rather than just leaving a textarea always
 *   present.
 * - `reject`/`escalate`: a required reason, same shape as
 *   `ApprovalDialog`'s rejection reason.
 */
export default function DecisionDialog({ decision, busy, onCancel, onConfirm }: Props) {
  const [outcome, setOutcome] = useState<VerificationOutcome>('Flawless');
  const [wantsComment, setWantsComment] = useState(false);
  const [comment, setComment] = useState('');
  const [reason, setReason] = useState('');

  const title = decision === 'verify' ? 'Verify list' : decision === 'reject' ? 'Reject list' : 'Escalate list';
  const reasonRequired = decision === 'reject' || decision === 'escalate';
  const confirmDisabled = busy || (reasonRequired && !reason.trim());

  function confirm() {
    if (decision === 'verify') {
      onConfirm({ outcome, comment: wantsComment ? comment : undefined });
    } else {
      onConfirm({ reason });
    }
  }

  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onCancel()}>
      <section className="dialog" role="dialog" aria-modal="true" aria-labelledby="decision-dialog-title">
        <h2 id="decision-dialog-title">{title}</h2>

        {decision === 'verify' ? (
          <>
            <fieldset style={{ border: 0, padding: 0, margin: '12px 0' }}>
              <legend className="muted" style={{ fontSize: 13 }}>
                Outcome
              </legend>
              <label style={{ display: 'block', marginTop: 6 }}>
                <input
                  type="radio"
                  name="outcome"
                  checked={outcome === 'Flawless'}
                  onChange={() => setOutcome('Flawless')}
                />{' '}
                Flawless
              </label>
              <label style={{ display: 'block', marginTop: 6 }}>
                <input
                  type="radio"
                  name="outcome"
                  checked={outcome === 'WithDeficiencies'}
                  onChange={() => setOutcome('WithDeficiencies')}
                />{' '}
                With deficiencies
              </label>
            </fieldset>
            <label style={{ display: 'block', marginTop: 12 }}>
              <input type="checkbox" checked={wantsComment} onChange={(e) => setWantsComment(e.target.checked)} /> Add
              a comment
            </label>
            {wantsComment ? (
              <textarea
                value={comment}
                onChange={(event) => setComment(event.target.value)}
                rows={4}
                autoFocus
                aria-label="Comment"
              />
            ) : null}
          </>
        ) : (
          <>
            <label htmlFor="decision-reason">Reason (required)</label>
            <textarea id="decision-reason" value={reason} onChange={(event) => setReason(event.target.value)} rows={4} autoFocus />
          </>
        )}

        <div className="dialog-actions">
          <button className="button-secondary" type="button" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            className={decision === 'reject' ? 'button-danger' : 'button-primary'}
            type="button"
            onClick={confirm}
            disabled={confirmDisabled}
          >
            {busy ? 'Submitting…' : `Confirm ${decision}`}
          </button>
        </div>
      </section>
    </div>
  );
}
