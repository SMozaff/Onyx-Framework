import { useEffect } from 'react';
import { usePushStore } from '../stores/pushStore';
import { useI18n } from '../i18n/I18nContext';

/**
 * Web Push opt-in/opt-out card (MIGRATION_PLAN Phase 3.2). Rendered on the
 * Notifications view. The card only ever requests permission and registers
 * a subscription — it performs no command and cannot deliver anything.
 */
export function PushNotificationsCard() {
  const { t } = useI18n();
  const { status, endpoint, pending, message, boot, enable, disable } = usePushStore();

  useEffect(() => {
    void boot();
  }, [boot]);

  const actionable = status === 'idle' || status === 'error';
  const subscribed = status === 'subscribed';

  return (
    <section
      aria-label={t("notifications.pushNotifications")}
      className="rounded-lg border border-slate-200 bg-white p-4 text-sm"
    >
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="font-medium text-slate-900">{t("notifications.webPushDelivery")}</p>
          {subscribed && endpoint ? (
            <p className="truncate text-xs text-slate-500">{endpoint}</p>
          ) : null}
        </div>
        {subscribed ? (
          <button
            type="button"
            className="rounded border border-slate-300 px-3 py-1.5 text-slate-700 hover:bg-slate-50 disabled:cursor-not-allowed disabled:opacity-60"
            onClick={() => void disable()}
            disabled={pending}
          >
            {pending ? t("common.disabling") : t("common.disable")}
          </button>
        ) : (
          <button
            type="button"
            className="rounded bg-[#0a1e3d] px-3 py-1.5 text-white hover:bg-[#122a4d] disabled:cursor-not-allowed disabled:opacity-60"
            onClick={() => void enable()}
            disabled={pending || !actionable}
          >
            {pending ? t("common.enabling") : t("notifications.enable")}
          </button>
        )}
      </div>

      {status === 'unsupported' && (
        <p className="mt-2 text-xs text-slate-500">
          {t("notifications.pushUnsupported")}
        </p>
      )}
      {status === 'unconfigured' && (
        <p className="mt-2 text-xs text-slate-500">
          {t("notifications.pushUnconfigured")}
        </p>
      )}
      {status === 'denied' && (
        <p className="mt-2 text-xs text-slate-500">
          {t("notifications.pushDenied")}
        </p>
      )}
      {status === 'error' && message ? (
        <p className="mt-2 text-xs text-red-700" role="alert">
          {message}
        </p>
      ) : null}

      <p className="mt-2 text-xs text-slate-400">
        {t("notifications.pushReadOnlyNote")}
      </p>
    </section>
  );
}