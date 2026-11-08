import { useI18n } from "@/i18n/I18nContext";

/**
 * Route selector for file transfer. Only Internet is available now. LAN and the
 * emergency BLE/Wi-Fi Direct module are shown disabled as placeholders (owner
 * decision, 2026-11-07).
 */
export function TransferRoutes() {
  const { t } = useI18n();
  const routes = [
    { id: "internet", label: t("files.routeInternet"), note: t("files.routeInternetNote"), enabled: true },
    { id: "lan", label: t("files.routeLan"), note: t("files.routeComingSoon"), enabled: false },
    { id: "emergency", label: t("files.routeEmergency"), note: t("files.routeFutureModule"), enabled: false },
  ];

  return (
    <section className="mb-6 rounded-lg border border-onyx-border bg-onyx-surface p-5">
      <h2 className="text-sm font-medium text-onyx-text">{t("files.routeTitle")}</h2>
      <ul className="mt-3 space-y-2">
        {routes.map((route) => (
          <li key={route.id}>
            <button
              type="button"
              disabled={!route.enabled}
              aria-disabled={!route.enabled}
              aria-pressed={route.enabled ? true : undefined}
              className="flex w-full items-center justify-between rounded border border-onyx-border px-3 py-2 text-start text-sm text-onyx-text disabled:cursor-not-allowed disabled:text-onyx-text-dim"
            >
              <span>{route.label}</span>
              <span className="text-xs text-onyx-text-dim">{route.note}</span>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
