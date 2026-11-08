/**
 * Display-only digit formatting for identifiers (decision Q-16 / U-05).
 *
 * Persian (U+06F0–U+06F9) and Arabic-Indic (U+0660–U+0669) digits are shown as
 * Latin 0–9. This never changes the stored value: canonical data, serialization,
 * identifiers, and API payloads keep the original characters. Use only when
 * rendering; never pass the result back to the API or store it.
 */
const PERSIAN_DIGITS = "۰۱۲۳۴۵۶۷۸۹";
const ARABIC_INDIC_DIGITS = "٠١٢٣٤٥٦٧٨٩";

export function toLatinDigits(value: string): string {
  let out = "";
  for (const ch of value) {
    const p = PERSIAN_DIGITS.indexOf(ch);
    const a = ARABIC_INDIC_DIGITS.indexOf(ch);
    if (p !== -1) out += String(p);
    else if (a !== -1) out += String(a);
    else out += ch;
  }
  return out;
}

// Unicode isolates keep an identifier LTR inside RTL text, so separators such as
// "-" do not move to the wrong side. Applied by IdText only, not by toLatinDigits.
const LRI = "⁦";
const PDI = "⁩";

export function isolateLtr(text: string): string {
  return `${LRI}${text}${PDI}`;
}
