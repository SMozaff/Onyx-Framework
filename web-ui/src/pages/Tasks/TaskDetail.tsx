import { useI18n } from '../../i18n/I18nContext';
import StatusBadge from '../../components/StatusBadge';
import type { TaskSummary } from '../../types/query';
export default function TaskDetail({
task }: { task: TaskSummary }) {
  const { t } = useI18n(); return <section className="detail-panel" aria-labelledby="task-title"><div className="detail-header"><div><p className="eyebrow">{t('tasks.singular')}</p><h2 id="task-title">{task.title}</h2></div><StatusBadge status={task.status} /></div><dl className="detail-grid"><div><dt>{t('common.owner')}</dt><dd>{task.owner}</dd></div><div><dt>{t('common.priority')}</dt><dd>{task.priority}</dd></div><div><dt>{t('common.due')}</dt><dd>{new Date(task.due_at).toLocaleString()}</dd></div><div><dt>{t('common.version')}</dt><dd>{task.version}</dd></div><div><dt>{t('missions.singular')}</dt><dd className="mono">{task.mission_id}</dd></div><div><dt>{t('common.updated')}</dt><dd>{new Date(task.updated_at).toLocaleString()}</dd></div></dl><div className="read-only-note"><strong>{t('common.readOnlyScope')}</strong><span>{t('tasks.nativeClientRequired')}</span></div></section>; }
