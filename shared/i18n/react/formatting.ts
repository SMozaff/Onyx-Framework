export type DateTimeInput = Date | number | string;
export type RelativeTimeUnit =
  | "year"
  | "years"
  | "quarter"
  | "quarters"
  | "month"
  | "months"
  | "week"
  | "weeks"
  | "day"
  | "days"
  | "hour"
  | "hours"
  | "minute"
  | "minutes"
  | "second"
  | "seconds";
export type DurationUnit = "millisecond" | "second" | "minute" | "hour" | "day";
export type DurationUnitDisplay = "long" | "short" | "narrow";

export interface DurationFormatOptions {
  unitDisplay?: DurationUnitDisplay;
}

export interface Formatters {
  formatDate(value: DateTimeInput, options?: Intl.DateTimeFormatOptions): string;
  formatDateTime(value: DateTimeInput, options?: Intl.DateTimeFormatOptions): string;
  formatRelativeTime(value: number, unit: RelativeTimeUnit, options?: Intl.RelativeTimeFormatOptions): string;
  formatNumber(value: number, options?: Intl.NumberFormatOptions): string;
  formatPercent(value: number, options?: Intl.NumberFormatOptions): string;
  formatCurrency(value: number, currency: string, options?: Intl.NumberFormatOptions): string;
  formatDuration(value: number, unit?: DurationUnit, options?: DurationFormatOptions): string;
}

/**
 * Create the canonical locale-aware formatting API.
 *
 * Callers supply an already-resolved BCP 47 locale, normally
 * `localeMetadata[locale].formatLocale`. The implementation deliberately
 * contains no per-language branches: calendars, digits, plural-sensitive
 * units, and list conjunctions all come from `Intl`.
 */
export function createFormatters(formatLocale: string): Formatters {
  const locale = normalizeFormatLocale(formatLocale);

  return {
    formatDate(value, options) {
      return new Intl.DateTimeFormat(locale, options).format(toDate(value));
    },
    formatDateTime(value, options = { dateStyle: "medium", timeStyle: "short" }) {
      return new Intl.DateTimeFormat(locale, options).format(toDate(value));
    },
    formatRelativeTime(value, unit, options = { numeric: "auto" }) {
      return new Intl.RelativeTimeFormat(locale, options).format(toFiniteNumber(value, "value"), unit);
    },
    formatNumber(value, options) {
      return new Intl.NumberFormat(locale, options).format(toFiniteNumber(value, "value"));
    },
    formatPercent(value, options) {
      return new Intl.NumberFormat(locale, { ...options, style: "percent" }).format(
        toFiniteNumber(value, "value"),
      );
    },
    formatCurrency(value, currency, options) {
      return new Intl.NumberFormat(locale, {
        ...options,
        style: "currency",
        currency: normalizeCurrencyCode(currency),
      }).format(toFiniteNumber(value, "value"));
    },
    formatDuration(value, unit = "second", options = {}) {
      return formatDuration(locale, toFiniteNumber(value, "value"), unit, options.unitDisplay ?? "long");
    },
  };
}

function normalizeFormatLocale(value: string): string {
  const locale = value.trim();
  if (locale.length === 0) {
    throw new TypeError("A BCP 47 format locale is required.");
  }
  try {
    return Intl.getCanonicalLocales(locale)[0] ?? locale;
  } catch {
    throw new RangeError(`Unsupported format locale: ${value}`);
  }
}

function toDate(value: DateTimeInput): Date {
  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) {
    throw new RangeError("A valid date is required.");
  }
  return date;
}

function toFiniteNumber(value: number, label: string): number {
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new TypeError(`A finite ${label} is required.`);
  }
  return value;
}

function normalizeCurrencyCode(value: string): string {
  const currency = value.trim().toUpperCase();
  if (!/^[A-Z]{3}$/.test(currency)) {
    throw new TypeError("A three-letter ISO 4217 currency code is required.");
  }
  return currency;
}

function formatDuration(
  locale: string,
  value: number,
  unit: DurationUnit,
  unitDisplay: DurationUnitDisplay,
): string {
  const totalSeconds = value * unitInSeconds(unit);
  const sign = totalSeconds < 0 ? -1 : 1;
  let remaining = Math.abs(totalSeconds);
  const days = Math.floor(remaining / 86_400);
  remaining -= days * 86_400;
  const hours = Math.floor(remaining / 3_600);
  remaining -= hours * 3_600;
  const minutes = Math.floor(remaining / 60);
  const seconds = remaining - minutes * 60;
  const parts: Array<{ amount: number; unit: Intl.NumberFormatOptions["unit"] }> = [];
  if (days > 0) parts.push({ amount: sign * days, unit: "day" });
  if (hours > 0) parts.push({ amount: sign * hours, unit: "hour" });
  if (minutes > 0) parts.push({ amount: sign * minutes, unit: "minute" });
  if (seconds > 0 || parts.length === 0) parts.push({ amount: sign * seconds, unit: "second" });

  const formatted = parts.map(({ amount, unit: partUnit }) =>
    new Intl.NumberFormat(locale, { style: "unit", unit: partUnit, unitDisplay }).format(amount),
  );
  return new Intl.ListFormat(locale, { style: "long", type: "conjunction" }).format(formatted);
}

function unitInSeconds(unit: DurationUnit): number {
  switch (unit) {
    case "millisecond":
      return 1 / 1_000;
    case "second":
      return 1;
    case "minute":
      return 60;
    case "hour":
      return 3_600;
    case "day":
      return 86_400;
  }
}
