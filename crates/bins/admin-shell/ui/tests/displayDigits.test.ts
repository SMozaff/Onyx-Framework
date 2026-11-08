import { describe, expect, it } from "vitest";
import { isolateLtr, toLatinDigits } from "../src/utils/displayDigits";

describe("toLatinDigits (display only)", () => {
  it("converts Persian digits to Latin", () => {
    expect(toLatinDigits("st۲۶sm۰۰۳۵")).toBe("st26sm0035");
  });

  it("converts Arabic-Indic digits to Latin", () => {
    expect(toLatinDigits("tm٢٦١١٠٧")).toBe("tm261107");
  });

  it("leaves Latin text and separators untouched", () => {
    expect(toLatinDigits("st26sm0035")).toBe("st26sm0035");
    expect(toLatinDigits("tm261107-AB")).toBe("tm261107-AB");
  });

  it("is idempotent", () => {
    const once = toLatinDigits("۱۲۳abc");
    expect(toLatinDigits(once)).toBe(once);
  });

  it("keeps Persian letters that are not digits", () => {
    expect(toLatinDigits("کارمند ۴")).toBe("کارمند 4");
  });

  it("does not alter the original input string", () => {
    const input = "st۲۶";
    toLatinDigits(input);
    expect(input).toBe("st۲۶");
  });
});

describe("isolateLtr (mixed RTL/LTR)", () => {
  it("wraps text in LRI and PDI so it stays LTR inside RTL text", () => {
    expect(isolateLtr("st26sm0035")).toBe("⁦st26sm0035⁩");
  });

  it("keeps the wrapped text unchanged between the controls", () => {
    const wrapped = isolateLtr("tm261107-AB");
    expect(wrapped.slice(1, -1)).toBe("tm261107-AB");
  });
});
