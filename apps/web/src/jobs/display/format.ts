import { BYTE_UNITS } from "./conts";
const number = new Intl.NumberFormat("ru-RU");
export const formatCount = (value: string | null | undefined): string =>
  value === null || value === undefined
    ? "Не проверено"
    : number.format(BigInt(value));
export const formatBytes = (value: string | null | undefined): string => {
  if (value === null || value === undefined) return "—";
  const bytes = BigInt(value);
  const unit = BYTE_UNITS.find(([, scale]) => bytes >= scale);
  if (!unit) return `${number.format(bytes)} Б`;
  const [label, scale] = unit;
  const whole = bytes / scale;
  const decimal = ((bytes % scale) * 10n) / scale;
  return `${number.format(whole)},${decimal} ${label}`;
};
