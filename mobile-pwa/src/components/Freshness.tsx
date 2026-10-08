import { useI18n } from '../i18n/I18nContext';

interface FreshnessProps {
  state: { kind: string };
}

export function Freshness({ state }: FreshnessProps) {
  const { t } = useI18n();
  if (state.kind === 'stale') {
    return (
      <span className="rounded-full bg-amber-100 px-2 py-0.5 text-xs font-medium text-amber-800">
        {t("status.stale")}
      </span>
    );
  }
  return (
    <span className="rounded-full bg-emerald-100 px-2 py-0.5 text-xs font-medium text-emerald-800">
      {t("status.active")}
    </span>
  );
}