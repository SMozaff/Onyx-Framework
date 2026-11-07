import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/I18nContext";

export type Theme = "light" | "dark";

const STORAGE_KEY = "onyx-admin-theme";

/** Reads the saved theme. Light when nothing is saved or storage is blocked. */
function readStoredTheme(): Theme {
  try {
    return window.localStorage.getItem(STORAGE_KEY) === "dark" ? "dark" : "light";
  } catch {
    return "light";
  }
}

/**
 * Light/dark switch for the admin shell. Sets `data-theme` on the root element,
 * which activates the dark tokens in index.css. The choice is remembered on
 * this device; if storage is unavailable the theme still applies for the session.
 */
export function ThemeSwitcher() {
  const { t } = useI18n();
  const [theme, setTheme] = useState<Theme>(readStoredTheme);

  useEffect(() => {
    document.documentElement.dataset.theme = theme;
    try {
      window.localStorage.setItem(STORAGE_KEY, theme);
    } catch {
      // Storage blocked: keep the in-session theme.
    }
  }, [theme]);

  return (
    <label className="inline-flex items-center gap-2 text-xs font-semibold text-onyx-text">
      <span>{t("theme.title")}</span>
      <select
        value={theme}
        onChange={(event) => setTheme(event.target.value as Theme)}
        className="rounded border border-onyx-border bg-onyx-surface px-2 py-1 text-onyx-text"
      >
        <option value="light">{t("theme.light")}</option>
        <option value="dark">{t("theme.dark")}</option>
      </select>
    </label>
  );
}
