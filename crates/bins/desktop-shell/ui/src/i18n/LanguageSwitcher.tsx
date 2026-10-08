import { useI18n, type Locale } from "./I18nContext";

const OPTIONS: Array<{ value: Locale; label: string }> = [
  { value: "en", label: "English" },
  { value: "fa", label: "فارسی" },
];

/**
 * Compact EN/FA toggle. Drop into any topbar / settings screen.
 * `className` passthrough lets each app reuse its own button styles.
 */
export function LanguageSwitcher({
  className,
  selectClassName,
  label,
}: {
  className?: string;
  selectClassName?: string;
  label?: string;
}) {
  const { locale, setLocale, t } = useI18n();
  const visibleLabel = label ?? t("language.title");

  return (
    <label
      className={className ?? "language-switcher"}
      style={{ display: "inline-flex", alignItems: "center", gap: 8 }}
    >
      <span aria-hidden={false} style={{ fontSize: 12, fontWeight: 700 }}>
        {visibleLabel}
      </span>
      <select
        aria-label={t("common.language")}
        className={selectClassName}
        value={locale}
        onChange={(e) => setLocale(e.target.value as Locale)}
        style={{ width: "auto", margin: 0, padding: "6px 10px" }}
      >
        {OPTIONS.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </label>
  );
}
