import StatusBadge from '../../components/StatusBadge';
import type { MissionSummary } from '../../types/query';

export default function MissionList({ missions, selectedId, onSelect }: { missions: MissionSummary[]; selectedId: string | null; onSelect: (mission: MissionSummary) => void }) {
  return <div className="data-list" role="list" aria-label="Missions">{missions.map((mission) => <button type="button" role="listitem" key={mission.id} className={`data-row ${selectedId === mission.id ? 'data-row-selected' : ''}`} onClick={() => onSelect(mission)}><div><strong>{mission.name}</strong><span>{mission.owner}</span></div><div className="row-meta"><span>{mission.progress}%</span><StatusBadge status={mission.status} /></div></button>)}</div>;
}
