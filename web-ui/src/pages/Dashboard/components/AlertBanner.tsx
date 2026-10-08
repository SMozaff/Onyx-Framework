import { useI18n } from '../../../i18n/I18nContext';
import type { DashboardProjection } from '../../../types/query';

export default function AlertBanner({ data }: { data: DashboardProjection }) {
  const { t } = useI18n();
  if (!data.blocked_tasks && !data.unread_notifications) {
    return <div className="alert-banner alert-success"><strong>{t('dashboard.operationsStable')}</strong><span>{t('dashboard.noBlockedAlerts')}</span></div>;
  }
  return <div className="alert-banner"><strong>{t('dashboard.attentionRequired')}</strong><span>{t('dashboard.blockedAlertsNeedReview', { blocked: data.blocked_tasks, unread: data.unread_notifications })}</span></div>;
}
