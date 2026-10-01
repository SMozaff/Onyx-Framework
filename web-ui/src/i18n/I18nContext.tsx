import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { defaultLocale, dictionaries, en, localeIds, localeMetadata } from "./dictionaries.generated";
import type { Locale } from "./dictionaries.generated";
import { createFormatters, type Formatters } from "./formatting";
import { pluralCategory as resolvePluralCategory, type PluralCategory } from "./plural";

export type { Locale };
export const LOCALE_STORAGE_KEY = "onyx.locale";
export const DEFAULT_LOCALE: Locale = defaultLocale;

export type Dictionary = Record<string, string>;

interface I18nContextValue {
  locale: Locale;
  dir: "ltr" | "rtl";
  isRTL: boolean;
  formatLocale: string;
  format: Formatters;
  pluralCategory: (count: number) => PluralCategory;
  t: (key: string, vars?: Record<string, string | number>) => string;
  setLocale: (locale: Locale) => void;
}

const I18nContext = createContext<I18nContextValue | null>(null);

function isLocale(value: unknown): value is Locale {
  return typeof value === "string" && (localeIds as readonly string[]).includes(value);
}

function metadataFor(locale: Locale) {
  return localeMetadata[locale] ?? localeMetadata[DEFAULT_LOCALE];
}

function matchBrowserLocale(language: string): Locale | null {
  const normalized = language.trim().toLowerCase();
  if (normalized.length === 0) return null;
  for (const locale of localeIds) {
    if (!isLocale(locale)) continue;
    const formatLocale = metadataFor(locale).formatLocale.toLowerCase();
    const candidates = [locale, formatLocale];
    if (candidates.some((candidate) => normalized === candidate || normalized.startsWith(`${candidate}-`))) {
      return locale;
    }
  }
  return null;
}

export function detectInitialLocale(): Locale {
  try {
    const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (isLocale(stored)) return stored;
  } catch {
    /* storage unavailable (private mode) — fall through */
  }
  const browserLanguage = typeof navigator !== "undefined" ? navigator.language : "";
  return matchBrowserLocale(browserLanguage) ?? DEFAULT_LOCALE;
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
  const metadata = metadataFor(locale);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.documentElement.dir = metadata.direction;
    try {
      localStorage.setItem(LOCALE_STORAGE_KEY, locale);
    } catch {
      /* ignore */
    }
  }, [locale, metadata.direction]);

  const setLocale = useCallback((next: Locale) => {
    if (!isLocale(next)) return;
    setLocaleState(next);
  }, []);

  const format = useMemo(() => createFormatters(metadata.formatLocale), [metadata.formatLocale]);
  const value = useMemo<I18nContextValue>(() => {
    const active = dictionaries[locale] ?? dictionaries[DEFAULT_LOCALE] ?? en;
    const fallback = dictionaries[DEFAULT_LOCALE] ?? en;
    const t = (key: string, vars?: Record<string, string | number>) => {
      const hit = active[key] ?? fallback[key];
      if (hit === undefined) return "";
      return interpolate(hit, vars);
    };
    return {
      locale,
      dir: metadata.direction,
      isRTL: metadata.direction === "rtl",
      formatLocale: metadata.formatLocale,
      format,
      pluralCategory: (count: number) => resolvePluralCategory(locale, count),
      t,
      setLocale,
    };
  }, [locale, dictionaries, format, metadata, setLocale]);

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
  const hit = dictionaries[locale]?.[key] ?? dictionaries[DEFAULT_LOCALE]?.[key] ?? en[key];
  if (hit === undefined) return "";
  return interpolate(hit, vars);
}
