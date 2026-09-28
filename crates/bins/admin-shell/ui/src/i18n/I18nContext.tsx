import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { en } from "./dictionaries.generated";

export type Locale = "en" | "fa";
export const LOCALE_STORAGE_KEY = "onyx.locale";
export const DEFAULT_LOCALE: Locale = "en";

export type Dictionary = Record<string, string>;

interface I18nContextValue {
  locale: Locale;
  dir: "ltr" | "rtl";
  isRTL: boolean;
  t: (key: string, vars?: Record<string, string | number>) => string;
  setLocale: (locale: Locale) => void;
}

const I18nContext = createContext<I18nContextValue | null>(null);

export function detectInitialLocale(): Locale {
  try {
    const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (stored === "fa" || stored === "en") return stored;
  } catch {
    /* storage unavailable (private mode) — fall through */
  }
  const nav = typeof navigator !== "undefined" ? navigator.language.toLowerCase() : "";
  if (nav.startsWith("fa")) return "fa";
  return DEFAULT_LOCALE;
}

function interpolate(template: string, vars?: Record<string, string | number>): string {
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (m, k) => (vars[k] !== undefined ? String(vars[k]) : m));
}

export function LocaleProvider({
  children,
  dictionaries,
}: {
  children: ReactNode;
  dictionaries: Record<Locale, Dictionary>;
}) {
  const [locale, setLocaleState] = useState<Locale>(() => detectInitialLocale());

  useEffect(() => {
    document.documentElement.lang = locale === "fa" ? "fa" : "en";
    document.documentElement.dir = locale === "fa" ? "rtl" : "ltr";
    try {
      localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    } catch {
      /* ignore */
    }
  }, [locale]);

  const setLocale = useCallback((next: Locale) => setLocaleState(next), []);

  const value = useMemo<I18nContextValue>(() => {
    const active = dictionaries[locale] ?? dictionaries.en ?? en;
    const fallback = dictionaries.en ?? en;
    const t = (key: string, vars?: Record<string, string | number>) => {
      const hit = active[key] ?? fallback[key];
      if (hit === undefined) return key;
      return interpolate(hit, vars);
    };
    return {
      locale,
      dir: locale === "fa" ? "rtl" : "ltr",
      isRTL: locale === "fa",
      t,
      setLocale,
    };
  }, [locale, dictionaries, setLocale]);

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18nContextValue {
  const ctx = useContext(I18nContext);
  if (!ctx) throw new Error("useI18n must be used inside <LocaleProvider>");
  return ctx;
}

/** Non-hook accessor for places without React context (event handlers, utils). */
export function translateStatic(
  key: string,
  locale: Locale,
  dictionaries: Record<Locale, Dictionary>,
  vars?: Record<string, string | number>,
): string {
  const hit = dictionaries[locale]?.[key] ?? dictionaries.en?.[key] ?? en[key];
  if (hit === undefined) return key;
  return interpolate(hit, vars);
}
