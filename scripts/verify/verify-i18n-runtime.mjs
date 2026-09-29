/**
 * Unit tests for the canonical localization runtime API (Phase 1, P1-6/P1-8).
 *
 * Covers `shared/i18n/react/formatting.ts` and `shared/i18n/react/plural.ts`.
 * These modules are canonical TypeScript with erasable syntax only, so they are
 * loaded through `module.stripTypeScriptTypes` instead of a build step. That
 * keeps the check runnable from the standalone `i18n` CI job, which must stay
 * independent of frontend linting, type-checking, and test jobs.
 *
 * Usage: node --test scripts/verify/verify-i18n-runtime.mjs
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { stripTypeScriptTypes } from "node:module";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repositoryRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const reactDirectory = join(repositoryRoot, "shared", "i18n", "react");

if (typeof stripTypeScriptTypes !== "function") {
  throw new Error(
    "module.stripTypeScriptTypes is unavailable; Node 22.13 or newer is required to verify the localization runtime.",
  );
}

async function importCanonicalModule(fileName) {
  const source = readFileSync(join(reactDirectory, fileName), "utf8");
  const stripped = stripTypeScriptTypes(source, { mode: "strip" });
  const dataUrl = `data:text/javascript;base64,${Buffer.from(stripped, "base64")}`;
  return import(dataUrl);
}

const { createFormatters } = await importCanonicalModule("formatting.ts");
const { pluralCategory, selectPluralForm } = await importCanonicalModule("plural.ts");

const EN = createFormatters("en-US");
const FA = createFormatters("fa-IR");

const REQUIRED_FORMATTERS = [
  "formatDate",
  "formatDateTime",
  "formatRelativeTime",
  "formatNumber",
  "formatPercent",
  "formatCurrency",
  "formatDuration",
];

test("createFormatters exposes the full canonical formatting API", () => {
  for (const name of REQUIRED_FORMATTERS) {
    assert.equal(typeof EN[name], "function", `${name} must be exposed`);
  }
});

test("formatting is locale-parameterized rather than branch-per-language", (t) => {
  if (Intl.NumberFormat.supportedLocalesOf(["fa-IR"]).length === 0) {
    t.skip("fa-IR numbering data is unavailable in this ICU build");
    return;
  }
  assert.notEqual(
    EN.formatNumber(1234567.891),
    FA.formatNumber(1234567.891),
    "the same value must not be formatted identically for en-US and fa-IR",
  );
  assert.equal(EN.formatNumber(1234567.891, { style: "percent" }), "123,456,789.1%");
});

test("formatDate and formatDateTime render and reject invalid input", () => {
  const reference = new Date(Date.UTC(2026, 8, 13, 10, 30, 0));
  assert.notEqual(EN.formatDate(reference), "");
  assert.notEqual(EN.formatDateTime(reference), "");
  assert.notEqual(EN.formatDate(reference, { dateStyle: "long" }), EN.formatDate(reference));
  assert.throws(() => EN.formatDate("not-a-date"), RangeError);
  assert.throws(() => EN.formatDateTime(Number.NaN), RangeError);
});

test("formatRelativeTime honours plural-sensitive units from Intl", () => {
  assert.equal(EN.formatRelativeTime(-1, "day"), "yesterday");
  assert.equal(EN.formatRelativeTime(0, "day"), "today");
  assert.notEqual(EN.formatRelativeTime(-3, "hour"), "");
  assert.throws(() => EN.formatRelativeTime(Number.POSITIVE_INFINITY, "day"), TypeError);
});

test("formatNumber, formatPercent, and formatCurrency validate their inputs", () => {
  assert.equal(EN.formatNumber(0.5, { minimumFractionDigits: 1 }), "0.5");
  assert.equal(EN.formatPercent(0.25), "25%");
  assert.equal(EN.formatCurrency(1234.5, "usd", { currencyDisplay: "code" }), "USD 1,234.50");
  assert.throws(() => EN.formatNumber("12"), TypeError);
  assert.throws(() => EN.formatPercent(Number.NaN), TypeError);
  assert.throws(() => EN.formatCurrency(1, "US"), TypeError);
});

test("formatDuration decomposes into localized unit phrases", () => {
  assert.equal(EN.formatDuration(1), "1 second");
  assert.notEqual(EN.formatDuration(0), "1 second");
  const composed = EN.formatDuration(90, "second", { unitDisplay: "short" });
  assert.match(composed, /1 min/);
  assert.match(composed, /30 sec/);
  assert.throws(() => EN.formatDuration(Number.NaN), TypeError);
});

test("createFormatters rejects an unusable format locale", () => {
  assert.throws(() => createFormatters(""), TypeError);
  assert.throws(() => createFormatters("not a locale"), RangeError);
});

test("pluralCategory agrees with Intl.PluralRules for every supported locale", () => {
  for (const locale of ["en", "fa", "en-US", "fa-IR"]) {
    const rules = new Intl.PluralRules(locale);
    for (const count of [0, 1, 2, 3, 7, 11, 21, 100, 1.5]) {
      assert.equal(
        pluralCategory(locale, count),
        rules.select(count),
        `pluralCategory(${locale}, ${count}) must match Intl.PluralRules`,
      );
    }
  }
});

test("pluralCategory validates its inputs", () => {
  assert.throws(() => pluralCategory("", 1), TypeError);
  assert.throws(() => pluralCategory("not a locale", 1), RangeError);
  assert.throws(() => pluralCategory("en", Number.NaN), TypeError);
});

test("selectPluralForm falls back to other when a category is untranslated", () => {
  const forms = { one: "one item", other: "{count} items" };
  assert.equal(selectPluralForm("en", 1, forms), "one item");
  assert.equal(selectPluralForm("en", 5, forms), "{count} items");
  assert.equal(selectPluralForm("fa", 5, { other: "fallback" }), "fallback");
  assert.equal(selectPluralForm("en", 2, { other: "fallback" }), "fallback");
});
