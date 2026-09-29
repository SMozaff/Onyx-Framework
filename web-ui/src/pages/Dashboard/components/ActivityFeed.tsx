import { formatDistanceToNow } from 'date-fns';
import type { ActivityItem } from '../../../types/query';
import StatusBadge from '../../../components/StatusBadge';
import { useI18n } from '../../../i18n/I18nContext';

export default function ActivityFeed({ items }: { items: ActivityItem[] }) {
  const { t } = useI18n();
  return <div className="activity-list">{items.map((item) => <article key={`${item.type}-${item.id}`} className="activity-item"><div><span className="activity-type">{item.type}</span><strong>{item.title}</strong><small>{safeRelative(item.updated_at, t('common.recentlyUpdated'))}</small></div><StatusBadge status={item.status} /></article>)}</div>;
}

function safeRelative(value: string, fallback: string) {
  const date = new Date(value);
  return Number.isNaN(date.valueOf()) ? fallback : formatDistanceToNow(date, { addSuffix: true });
}
