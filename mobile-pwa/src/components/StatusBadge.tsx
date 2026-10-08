const labels: Record<string, string> = {
  active: 'Active',
  paused: 'Paused',
  blocked: 'Blocked',
  pending: 'Pending',
  approved: 'Approved',
  rejected: 'Rejected',
  acknowledged: 'Acknowledged',
  unacknowledged: 'Unacknowledged',
  critical: 'Critical',
  upcoming: 'Upcoming',
  draft: 'Draft',
  submitted: 'Submitted',
  teamleaderprechecked: 'Pre-checked',
  verified: 'Verified',
  escalated: 'Escalated',
  requested: 'Requested',
  declined: 'Declined',
  extended: 'Extended',
  ended: 'Ended',
  expired: 'Expired',
};

type Tone = 'success' | 'warning' | 'danger' | 'info' | 'neutral';

/** Tone mapping mirrors web-ui's `statusTone` contract exactly. */
export function statusTone(status: string): Tone {
  if (['active', 'approved', 'acknowledged', 'online', 'connected', 'complete', 'verified', 'extended'].includes(status)) return 'success';
  if (['paused', 'pending', 'blocked', 'connecting', 'upcoming', 'submitted', 'teamleaderprechecked', 'requested'].includes(status)) return 'warning';
  if (['rejected', 'halted', 'critical', 'disconnected', 'failed', 'escalated', 'declined'].includes(status)) return 'danger';
  if (['syncing', 'processing'].includes(status)) return 'info';
  return 'neutral';
}

const toneClasses: Record<Tone, string> = {
  success: 'bg-emerald-100 text-emerald-800',
  warning: 'bg-amber-100 text-amber-800',
  danger: 'bg-red-100 text-red-800',
  info: 'bg-sky-100 text-sky-800',
  neutral: 'bg-slate-200 text-slate-700',
};

export function StatusBadge({ status }: { status: string }) {
  const normalized = status.toLowerCase();
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${toneClasses[statusTone(normalized)]}`}
      aria-label={`Status: ${labels[normalized] ?? status}`}
    >
      {labels[normalized] ?? status}
    </span>
  );
}