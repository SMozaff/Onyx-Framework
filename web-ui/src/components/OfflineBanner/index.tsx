import { useEffect, useState } from 'react';
import { useI18n } from '../../i18n/I18nContext';

export default function OfflineBanner() {
  const { t } = useI18n();
  const [offline, setOffline] = useState(!navigator.onLine);
  useEffect(() => {
    const down = () => setOffline(true);
    const up = () => setOffline(false);
    window.addEventListener('offline', down);
    window.addEventListener('online', up);
    window.addEventListener('onyx:network-error', down);
    return () => {
      window.removeEventListener('offline', down);
      window.removeEventListener('online', up);
      window.removeEventListener('onyx:network-error', down);
    };
  }, []);
  if (!offline) return null;
  return <div className="offline-banner" role="alert"><strong>{t('common.connectionLost')}</strong> {t('common.commandsNotQueued')}</div>;
}
