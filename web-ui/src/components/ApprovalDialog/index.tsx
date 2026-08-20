import { useEffect, useId, useRef, useState } from 'react';
import type { ApprovalProjection } from '../../types/query';

interface Props {
  approval: ApprovalProjection;
  decision: 'approve' | 'reject';
  busy: boolean;
  error?: string | null;
  onCancel: () => void;
  onConfirm: (reason: string) => void;
}

const reasonPolicy = {
  approve: { required: false, label: 'Decision note (optional)' },
  reject: { required: true, label: 'Reason (required)' },
} as const;

function focusableElements(root: HTMLElement): HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>('button:not([disabled]), [href], textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'));
}

export default function ApprovalDialog({ approval, decision, busy, error, onCancel, onConfirm }: Props) {
  const [reason, setReason] = useState('');
  const dialogRef = useRef<HTMLElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const titleId = useId();
  const descriptionId = useId();
  const policy = reasonPolicy[decision];

  useEffect(() => {
    previousFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    textareaRef.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !busy) {
        event.preventDefault();
        onCancel();
        return;
      }
      if (event.key !== 'Tab' || !dialogRef.current) return;
      const elements = focusableElements(dialogRef.current);
      if (!elements.length) return;
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      previousFocusRef.current?.focus();
    };
  }, [busy, onCancel]);

  const canSubmit = !busy && (!policy.required || reason.trim().length > 0);
  return (
    <div className="dialog-backdrop" onMouseDown={(event) => event.target === event.currentTarget && !busy && onCancel()}>
      <section ref={dialogRef} className="dialog" role="dialog" aria-modal="true" aria-labelledby={titleId} aria-describedby={descriptionId}>
        <h2 id={titleId}>{decision === 'approve' ? 'Approve' : 'Reject'} request</h2>
        <p id={descriptionId}>{approval.title}</p>
        <label htmlFor="decision-reason">{policy.label}</label>
        <textarea ref={textareaRef} id="decision-reason" value={reason} onChange={(event) => setReason(event.target.value)} rows={4} aria-required={policy.required} />
        {error && <p className="form-error" role="alert">{error}</p>}
        <div className="dialog-actions">
          <button className="button-secondary" type="button" onClick={onCancel} disabled={busy}>Cancel</button>
          <button className={decision === 'approve' ? 'button-primary' : 'button-danger'} type="button" onClick={() => onConfirm(reason.trim())} disabled={!canSubmit}>
            {busy ? 'Submitting…' : decision === 'approve' ? 'Confirm approval' : 'Confirm rejection'}
          </button>
        </div>
      </section>
    </div>
  );
}
