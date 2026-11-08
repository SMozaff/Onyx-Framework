import { useSession } from "@/hooks/useSession";
import { useI18n } from "@/i18n/I18nContext";

/** Account-only settings. The service endpoint is fixed for every Staff client. */
export default function Settings({
  onRequireReauthentication,
}: {
  onRequireReauthentication: () => Promise<void>;
}) {
  const session = useSession();
  const { t } = useI18n();

  return (
    <div className="max-w-2xl">
      <h1 className="text-xl font-semibold text-onyx-text">{t("settings.title")}</h1>
      <section className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-semibold text-onyx-text">{t("common.account")}</h2>
        <p className="mt-2 text-sm text-onyx-text-dim">
          Signed in as <strong className="font-medium text-onyx-text">{session.username}</strong>.
        </p>
        <p className="mt-2 text-xs text-onyx-text-dim">
          {t("settings.cloudServiceNote")}
        </p>
        <button
          type="button"
          onClick={() => void onRequireReauthentication()}
          className="mt-5 rounded-md border border-onyx-border px-3 py-1.5 text-sm font-medium text-onyx-text"
        >
          {t("auth.logOut")}
        </button>
      </section>
      <section className="mt-6 rounded-lg border border-onyx-border bg-onyx-surface p-5">
        <h2 className="text-base font-semibold text-onyx-text">{t("settings.notificationsTitle")}</h2>
        <ul className="mt-3 space-y-2">
          {[t("settings.notificationsEmail"), t("settings.notificationsSms")].map((label) => (
            <li key={label}>
              <button
                type="button"
                disabled
                aria-disabled
                className="flex w-full items-center justify-between rounded border border-onyx-border px-3 py-2 text-start text-sm text-onyx-text-dim"
              >
                <span>{label}</span>
                <span className="text-xs">{t("settings.notificationsFutureModule")}</span>
              </button>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
