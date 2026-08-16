import { useState } from 'react';
import { useRequestStaffLoan } from '../../hooks/useCommand';

/**
 * Requests a new `StaffLoan`. Takes the three party ids as raw UUID
 * input rather than a user picker — this app has no user-search/lookup
 * surface yet (same limitation `TodoTargets/CreateListForm.tsx` notes
 * for `ManagerAssigned` list creation), so a real picker is a natural
 * follow-up once one exists, not a compromise specific to this form.
 */
export default function CreateLoanForm() {
  const request = useRequestStaffLoan();
  const [staffUserId, setStaffUserId] = useState('');
  const [realOwnerId, setRealOwnerId] = useState('');
  const [borrowingManagerId, setBorrowingManagerId] = useState('');
  const [startAt, setStartAt] = useState('');
  const [endAt, setEndAt] = useState('');

  const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
  const idsValid = [staffUserId, realOwnerId, borrowingManagerId].every((id) => uuidPattern.test(id.trim()));
  const windowValid =
    startAt && endAt && !Number.isNaN(new Date(startAt).getTime()) && !Number.isNaN(new Date(endAt).getTime()) &&
    new Date(endAt).getTime() > new Date(startAt).getTime();
  const canSubmit = idsValid && windowValid && realOwnerId.trim() !== borrowingManagerId.trim();

  function submit() {
    if (!canSubmit) return;
    request.mutate(
      {
        staff_user_id: staffUserId.trim(),
        real_owner_id: realOwnerId.trim(),
        borrowing_manager_id: borrowingManagerId.trim(),
        start_at_ms: new Date(startAt).getTime(),
        end_at_ms: new Date(endAt).getTime(),
      },
      {
        onSuccess: () => {
          setStaffUserId('');
          setRealOwnerId('');
          setBorrowingManagerId('');
          setStartAt('');
          setEndAt('');
        },
      },
    );
  }

  return (
    <section className="panel">
      <div className="panel-heading">
        <h2>Request a staff loan</h2>
      </div>
      <p className="muted" style={{ marginTop: 0 }}>
        Requires the real owner's approval before it takes effect — see the Loans list below once submitted.
      </p>
      <div style={{ display: 'grid', gap: 10 }}>
        <label>
          <span className="muted" style={{ fontSize: 13 }}>
            Staff member's user id (UUID)
          </span>
          <input value={staffUserId} onChange={(e) => setStaffUserId(e.target.value)} placeholder="00000000-0000-0000-0000-000000000000" />
        </label>
        <label>
          <span className="muted" style={{ fontSize: 13 }}>
            Real owner's user id (UUID)
          </span>
          <input value={realOwnerId} onChange={(e) => setRealOwnerId(e.target.value)} placeholder="00000000-0000-0000-0000-000000000000" />
        </label>
        <label>
          <span className="muted" style={{ fontSize: 13 }}>
            Borrowing manager's user id (UUID)
          </span>
          <input
            value={borrowingManagerId}
            onChange={(e) => setBorrowingManagerId(e.target.value)}
            placeholder="00000000-0000-0000-0000-000000000000"
          />
        </label>
        <div style={{ display: 'flex', gap: 10 }}>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Loan starts
            </span>
            <input type="datetime-local" value={startAt} onChange={(e) => setStartAt(e.target.value)} />
          </label>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Loan ends
            </span>
            <input type="datetime-local" value={endAt} onChange={(e) => setEndAt(e.target.value)} />
          </label>
        </div>
        <button type="button" className="button-primary" onClick={submit} disabled={request.isPending || !canSubmit}>
          {request.isPending ? 'Requesting…' : 'Request loan'}
        </button>
      </div>
    </section>
  );
}
