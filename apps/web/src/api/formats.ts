import { fullFormats } from "ajv-formats/dist/formats";
import type { Format } from "ajv";
import {
  CANONICAL_COUNTER,
  CANONICAL_UUID,
  PARTIAL_DATE_TIME,
  UINT64_MAX,
} from "./conts";

const validPartialDateTime = (value: string): boolean => {
  if (!PARTIAL_DATE_TIME.test(value)) return false;
  const format = fullFormats["date-time"];
  if (
    typeof format !== "object" ||
    ("type" in format && format.type === "number") ||
    ("async" in format && format.async === true) ||
    !("validate" in format) ||
    typeof format.validate !== "function"
  ) {
    throw new Error("Unsupported date-time validator");
  }
  // Ajv's optional generic type tag does not narrow this pinned synchronous string format.
  const validate = format.validate as (input: string) => boolean;
  return validate(`${value}Z`);
};
export const formats = {
  ...fullFormats,
  "canonical-uuid": CANONICAL_UUID,
  "uint64-decimal": (value: string) =>
    CANONICAL_COUNTER.test(value) && BigInt(value) <= UINT64_MAX,
  "partial-date-time": validPartialDateTime,
  // Schemars emits explicit integer minima/maxima alongside these annotations.
  uint8: true,
  uint16: true,
  uint32: true,
} satisfies Record<string, Format | true>;
