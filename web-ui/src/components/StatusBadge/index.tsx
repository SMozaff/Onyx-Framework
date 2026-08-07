import clsx from 'clsx';

const labels: Record<string, string> = {
  active: 'Active', paused: 'Paused', blocked: 'Blocked', pending: 'Pending',
  approved: 'Approved', rejected: 'Rejected', acknowledged: 'Acknowledged',
  unacknowledged: 'Unacknowledged', critical: 'Critical', upcoming: 'Upcoming',
  online: 'Online', connected: 'Live', connecting: 'Connecting', disconnected: 'Disconnected',
};

export function statusTone(status: string): 'success' | 'warning' | 'danger' | 'info' | 'neutral' {
  if (['active', 'approved', 'acknowledged', 'online', 'connected', 'complete'].includes(status)) return 'success';
  if (['paused', 'pending', 'blocked', 'connecting', 'upcoming'].includes(status)) return 'warning';
  if (['rejected', 'halted', 'critical', 'disconnected', 'failed'].includes(status)) return 'danger';
  if (['syncing', 'processing'].includes(status)) return 'info';
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
