import { formatDistanceToNow } from 'date-fns';
import type { ActivityItem } from '../../../types/query';
import StatusBadge from '../../../components/StatusBadge';
export default function ActivityFeed({ items }: { items: ActivityItem[] }) {
  return <div className="activity-list">{items.map((item) => <article key={`${item.type}-${item.id}`} className="activity-item"><div><span className="activity-type">{item.type}</span><strong>{item.title}</strong><small>{safeRelative(item.updated_at)}</small></div><StatusBadge status={item.status} /></article>)}</div>;
}
function safeRelative(value: string) { const date = new Date(value); return Number.isNaN(date.valueOf()) ? 'Recently updated' : formatDistanceToNow(date, { addSuffix: true }); }
