interface ProjectionStatePanelProps {
  resource: string;
  state: { kind: string };
  compact?: boolean;
}

const COPY: Record<string, { heading: string; body: string }> = {
  unavailable: {
    heading: 'Projection unavailable',
    body: 'The server could not serve this projection. Refresh, or retry shortly.',
  },
  stale: {
    heading: 'Stale snapshot',
    body: 'You are viewing the last cached snapshot. Activity on the server may be newer.',
  },
};

export function ProjectionStatePanel({ resource, state, compact }: ProjectionStatePanelProps) {
  const copy = COPY[state.kind];
  if (!copy) return null;
  return (
    <div
      className={`rounded-lg border ${state.kind === 'stale' ? 'border-amber-200 bg-amber-50' : 'border-red-200 bg-red-50'} px-4 py-3 text-sm ${compact ? 'mb-4' : ''}`}
      role="status"
    >
      <strong className="block font-semibold text-slate-900">
        {copy.heading} ({resource})
      </strong>
      <span className="text-slate-700">{copy.body}</span>
    </div>
  );
}