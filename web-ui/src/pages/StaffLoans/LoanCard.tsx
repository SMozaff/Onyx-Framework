import { useState } from 'react';
import StatusBadge from '../../components/StatusBadge';
import { useDecideStaffLoan } from '../../hooks/useCommand';
import type { StaffLoanProjection } from '../../types/query';

/**
 * One loan's card: the three parties, window, status, and whichever
 * actions the *current* user's relationship to this specific loan
 * makes available. Design doc §2.1's three approval gates are
 * reflected directly in which buttons render for which viewer:
 * - Approve/Decline: only the real owner, only while `Requested`.
 * - Extend: only the staff member being loaned, only while `Active`
 *   or `Extended` (their own approval is what extension means, per
 *   the confirmed rule — there is no separate "request extension"
 *   step for anyone else to approve).
 * - End early: either the real owner or the borrowing manager, while
 *   `Active` or `Extended` — the one action requiring no approval at
 *   all, so it's available to two roles rather than gated to one.
 *
 * These client-side checks are a UX convenience only, not the real
 * authorization boundary — the server enforces the actual rule
 * regardless of what buttons this component chooses to show (same
 * posture as `TodoTargets/ListDetail.tsx`'s D.4 note).
 */
export default function LoanCard({ loan, currentUserId }: { loan: StaffLoanProjection; currentUserId: string | null }) {
  const decide = useDecideStaffLoan();
  const [extending, setExtending] = useState(false);
  const [newEndAt, setNewEndAt] = useState('');
  const [declineReason, setDeclineReason] = useState<string | null>(null);

  const isRealOwner = currentUserId === loan.real_owner_id;
  const isBorrowingManager = currentUserId === loan.borrowing_manager_id;
  const isStaffMember = currentUserId === loan.staff_user_id;

  const canApproveOrDecline = isRealOwner && loan.status === 'Requested';
  const canExtend = isStaffMember && (loan.status === 'Active' || loan.status === 'Extended');
  const canEnd = (isRealOwner || isBorrowingManager) && (loan.status === 'Active' || loan.status === 'Extended');

  function confirmExtend() {
    const ms = new Date(newEndAt).getTime();
    if (Number.isNaN(ms)) return;
    decide.mutate({ loan, decision: 'extend', newEndAtMs: ms }, { onSuccess: () => setExtending(false) });
  }

  function confirmDecline() {
    decide.mutate({ loan, decision: 'decline', reason: declineReason ?? '' }, { onSuccess: () => setDeclineReason(null) });
  }

  return (
    <article className="notification-card">
      <div className="notification-content">
        <div className="card-title-row">
          <h3>
            Loan {isStaffMember ? '(you are on loan)' : isRealOwner ? '(your report)' : isBorrowingManager ? '(borrowed by you)' : ''}
          </h3>
          <StatusBadge status={loan.status} />
        </div>
        <dl className="inline-details">
          <div>
            <dt>Window starts</dt>
            <dd>{new Date(loan.window.start_at / 1_000_000).toLocaleString()}</dd>
          </div>
          <div>
            <dt>Window ends</dt>
            <dd>{new Date(loan.window.end_at / 1_000_000).toLocaleString()}</dd>
          </div>
        </dl>

        {canApproveOrDecline || canExtend || canEnd ? (
          <div className="approval-actions" style={{ marginTop: 12, flexWrap: 'wrap' }}>
            {canApproveOrDecline ? (
              <>
                <button
                  className="button-primary"
                  type="button"
                  disabled={decide.isPending}
                  onClick={() => decide.mutate({ loan, decision: 'approve' })}
                >
                  Approve
                </button>
                <button
                  className="button-danger"
                  type="button"
                  disabled={decide.isPending}
                  onClick={() => setDeclineReason(declineReason === null ? '' : null)}
                >
                  Decline
                </button>
              </>
            ) : null}
            {canExtend ? (
              <button
                className="button-secondary"
                type="button"
                disabled={decide.isPending}
                onClick={() => setExtending((value) => !value)}
              >
                Extend
              </button>
            ) : null}
            {canEnd ? (
              <button
                className="button-secondary"
                type="button"
                disabled={decide.isPending}
                onClick={() => decide.mutate({ loan, decision: 'end' })}
              >
                End early
              </button>
            ) : null}
          </div>
        ) : null}

        {declineReason !== null ? (
          <div style={{ marginTop: 10, display: 'grid', gap: 8 }}>
            <textarea
              value={declineReason}
              onChange={(e) => setDeclineReason(e.target.value)}
              placeholder="Reason for declining (required)"
              rows={2}
            />
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="button-secondary" type="button" onClick={() => setDeclineReason(null)}>
                Cancel
              </button>
              <button
                className="button-danger"
                type="button"
                disabled={decide.isPending || !declineReason.trim()}
                onClick={confirmDecline}
              >
                Confirm decline
              </button>
            </div>
          </div>
        ) : null}

        {extending ? (
          <div style={{ marginTop: 10, display: 'grid', gap: 8 }}>
            <label>
              <span className="muted" style={{ fontSize: 13 }}>
                New end date/time
              </span>
              <input type="datetime-local" value={newEndAt} onChange={(e) => setNewEndAt(e.target.value)} />
            </label>
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="button-secondary" type="button" onClick={() => setExtending(false)}>
                Cancel
              </button>
              <button className="button-primary" type="button" disabled={decide.isPending || !newEndAt} onClick={confirmExtend}>
                Confirm extension
              </button>
            </div>
          </div>
        ) : null}
      </div>
    </article>
  );
}
