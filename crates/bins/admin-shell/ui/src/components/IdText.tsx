import { toLatinDigits } from "@/utils/displayDigits";

/**
 * Renders an identifier for display: Latin digits and an LTR isolate so it reads
 * correctly inside RTL (Persian) text. The `value` prop is never modified.
 */
export function IdText({ value }: { value: string }) {
  return (
    <bdi dir="ltr" className="font-mono tabular-nums">
      {toLatinDigits(value)}
    </bdi>
  );
}
