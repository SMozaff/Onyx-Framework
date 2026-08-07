import StatusBadge from '../../components/StatusBadge';
import { useOnyxQuery } from '../../hooks/useQuery';
import type { MissionSummary, TimelineEntry } from '../../types/query';

export default function MissionDetail({ mission }: { mission: MissionSummary }) {
  const timeline = useOnyxQuery<TimelineEntry>('timeline.list', { subject_id: mission.id });
  return <section className="detail-panel" aria-labelledby="mission-title"><div className="detail-header"><div><p className="eyebrow">Mission</p><h2 id="mission-title">{mission.name}</h2></div><StatusBadge status={mission.status} /></div><p>{mission.summary}</p><dl className="detail-grid"><div><dt>Owner</dt><dd>{mission.owner}</dd></div><div><dt>Priority</dt><dd>{mission.priority}</dd></div><div><dt>Progress</dt><dd>{mission.progress}%</dd></div><div><dt>Version</dt><dd>{mission.version}</dd></div></dl><div className="progress-track" aria-label={`${mission.progress}% complete`}><span style={{ width: `${mission.progress}%` }} /></div><div className="detail-section"><h3>Timeline</h3>{timeline.data?.data.length ? timeline.data.data.map((item) => <article className="timeline-item" key={item.id}><div><strong>{item.label}</strong><span>{new Date(item.at).toLocaleString()}</span></div><StatusBadge status={item.status} /></article>) : <p className="muted">No timeline entries in this projection.</p>}</div></section>;
}
