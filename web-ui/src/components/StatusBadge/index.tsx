import clsx from 'clsx';

const labels: Record<string, string> = {
  active: 'Active', paused: 'Paused', blocked: 'Blocked', pending: 'Pending',
  approved: 'Approved', rejected: 'Rejected', acknowledged: 'Acknowledged',
  unacknowledged: 'Unacknowledged', critical: 'Critical', upcoming: 'Upcoming',
  online: 'Online', connected: 'Live', connecting: 'Connecting', disconnected: 'Disconnected',
  // TodoList/TargetList (todo_domain::state_machine::ListStatus), added
  // for the Todo/Target UI. Keys are lowercased since this component
  // normalizes `status` to lowercase before lookup — the wire value
  // itself is PascalCase ("TeamLeaderPreChecked", "Verified", etc.).
  draft: 'Draft', submitted: 'Submitted', teamleaderprechecked: 'Pre-checked',
  verified: 'Verified', escalated: 'Escalated',
  // StaffLoan (todo_domain::state_machine::StaffLoanStatus). 'active'
  // is already defined above (Mission/Task) and applies unchanged here
  // too — a loan being Active genuinely is the same "success" tone.
  requested: 'Requested', declined: 'Declined', extended: 'Extended',
  ended: 'Ended', expired: 'Expired',
};

export function statusTone(status: string): 'success' | 'warning' | 'danger' | 'info' | 'neutral' {
  if (['active', 'approved', 'acknowledged', 'online', 'connected', 'complete', 'verified', 'extended'].includes(status)) return 'success';
  if (['paused', 'pending', 'blocked', 'connecting', 'upcoming', 'submitted', 'teamleaderprechecked', 'requested'].includes(status)) return 'warning';
  if (['rejected', 'halted', 'critical', 'disconnected', 'failed', 'escalated', 'declined'].includes(status)) return 'danger';
  if (['syncing', 'processing'].includes(status)) return 'info';
  // 'draft' is intentionally absent from every bucket above and falls
  // through to 'neutral' below — a frozen wire-contract test
  // (tests/unit/contracts.test.ts) locks `statusTone('draft')` to
  // 'neutral'; adding it to 'info' here (an earlier draft of this
  // change did exactly that, before this test caught it) would violate
  // that contract. `TodoList`'s `Draft` status lowercases to the same
  // 'draft' key that Mission/Task already used, so no separate handling
  // is needed — the existing neutral fallback is already correct for
  // both. `StaffLoan`'s terminal 'Ended'/'Expired' also fall through to
  // neutral — deliberately not 'danger': ending a loan (early or on
  // schedule) is a normal outcome per design doc §2.1, not a failure.
  return 'neutral';
}

export default function StatusBadge({ status }: { status: string }) {
  const normalized = status.toLowerCase();
  return (
    <span className={clsx('status-badge', `status-${statusTone(normalized)}`)} aria-label={`Status: ${labels[normalized] ?? status}`}>
      <span className="status-dot" aria-hidden="true" />
      {labels[normalized] ?? status}
    </span>
  );
}
