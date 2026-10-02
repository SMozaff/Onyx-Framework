import { useOnline } from '../hooks/useOnline';
import { useI18n } from '../i18n/I18nContext';

export function OfflineBanner() {
  const { t } = useI18n();
  const online = useOnline();

  if (online) return null;

  return (
    <div
      role="status"
      data-testid="offline-banner"
      className="border-b border-amber-700 bg-amber-500 px-4 py-2 text-center text-sm font-medium text-white"
    >
      {t("common.offlineSnapshot")}
    </div>
  );
}