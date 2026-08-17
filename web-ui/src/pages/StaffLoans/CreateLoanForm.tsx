import { useState } from 'react';
import UserPicker from '../../components/UserPicker';
import { useRequestStaffLoan } from '../../hooks/useCommand';

/**
 * Requests a new `StaffLoan` using active same-organization identities from
 * the shared user picker rather than manually entered opaque UUIDs.
 */
export default function CreateLoanForm() {
  const request = useRequestStaffLoan();
  const [staffUserId, setStaffUserId] = useState('');
  const [realOwnerId, setRealOwnerId] = useState('');
  const [borrowingManagerId, setBorrowingManagerId] = useState('');
  const [startAt, setStartAt] = useState('');
  const [endAt, setEndAt] = useState('');

  const partyIdsSelected = Boolean(staffUserId && realOwnerId && borrowingManagerId);
  const windowValid =
    startAt && endAt && !Number.isNaN(new Date(startAt).getTime()) && !Number.isNaN(new Date(endAt).getTime()) &&
    new Date(endAt).getTime() > new Date(startAt).getTime();
  const canSubmit = partyIdsSelected && windowValid && realOwnerId !== borrowingManagerId;

  function submit() {
    if (!canSubmit) return;
    request.mutate(
      {
        staff_user_id: staffUserId,
        real_owner_id: realOwnerId,
        borrowing_manager_id: borrowingManagerId,
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
        <UserPicker
          label="Staff member"
          value={staffUserId}
          onChange={setStaffUserId}
          placeholder="Choose the staff member being loaned"
        />
        <UserPicker
          label="Real owner"
          value={realOwnerId}
          onChange={setRealOwnerId}
          placeholder="Choose the staff member's owning manager"
        />
        <UserPicker
          label="Borrowing manager"
          value={borrowingManagerId}
          onChange={setBorrowingManagerId}
          placeholder="Choose the manager receiving temporary authority"
        />
        {realOwnerId && realOwnerId === borrowingManagerId ? (
          <p role="alert" className="muted" style={{ margin: 0 }}>
            The real owner and borrowing manager must be different people.
          </p>
        ) : null}
        <div style={{ display: 'flex', gap: 10 }}>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Loan starts
            </span>
            <input type="datetime-local" value={startAt} onChange={(event) => setStartAt(event.target.value)} />
          </label>
          <label style={{ flex: 1 }}>
            <span className="muted" style={{ fontSize: 13 }}>
              Loan ends
            </span>
            <input type="datetime-local" value={endAt} onChange={(event) => setEndAt(event.target.value)} />
          </label>
        </div>
        <button type="button" className="button-primary" onClick={submit} disabled={request.isPending || !canSubmit}>
          {request.isPending ? 'Requesting…' : 'Request loan'}
        </button>
      </div>
    </section>
  );
}
