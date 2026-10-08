export type PluralCategory = "zero" | "one" | "two" | "few" | "many" | "other";
export type PluralForms<T> = { [Category in PluralCategory]?: T } & { other: T };

/**
 * Resolve the locale-aware CLDR plural category for a count.
 *
 * The category always comes from `Intl.PluralRules`; there are intentionally
 * no English/Persian branches here. Future locales work without changing this
 * implementation.
 */
export function pluralCategory(locale: string, count: number): PluralCategory {
  const normalizedLocale = normalizeLocale(locale);
  const normalizedCount = normalizeCount(count);
  return new Intl.PluralRules(normalizedLocale).select(normalizedCount) as PluralCategory;
}

/**
 * Choose a localized value from caller-supplied plural forms.
 *
 * `forms.other` is required because every locale's category set includes
 * `other`; it is also the safe fallback if a translation omits a category.
 * Dynamic interpolation of the returned value remains the caller's concern.
 */
export function selectPluralForm<T>(locale: string, count: number, forms: PluralForms<T>): T {
  const category = pluralCategory(locale, count);
  return forms[category] ?? forms.other;
}

function normalizeLocale(value: string): string {
  const locale = value.trim();
  if (locale.length === 0) {
    throw new TypeError("A BCP 47 locale is required.");
  }
  try {
    return Intl.getCanonicalLocales(locale)[0] ?? locale;
  } catch {
    throw new RangeError(`Unsupported locale: ${value}`);
  }
}

function normalizeCount(value: number): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new TypeError("A finite count is required.");
  }
  return value;
}
