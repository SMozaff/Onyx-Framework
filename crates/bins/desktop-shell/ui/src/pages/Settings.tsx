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
          ONYX uses its managed Cloud service for authentication and data.
        </p>
        <button
          type="button"
          onClick={() => void onRequireReauthentication()}
          className="mt-5 rounded-md border border-onyx-border px-3 py-1.5 text-sm font-medium text-onyx-text"
        >
          {t("auth.logOut")}
        </button>
      </section>
    </div>
  );
}
