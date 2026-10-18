export const TIME_LOCALE = "ru-RU";
export const DATE_TIME_OPTIONS = {
  dateStyle: "medium",
  timeStyle: "medium",
} as const satisfies Intl.DateTimeFormatOptions;
export const SERVER_DATE_TIME_OPTIONS = {
  ...DATE_TIME_OPTIONS,
  timeZone: "UTC",
} as const satisfies Intl.DateTimeFormatOptions;
export const UTC_TIME_OPTIONS = {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hourCycle: "h23",
  timeZone: "UTC",
} as const satisfies Intl.DateTimeFormatOptions;
export const MISSING_TIMESTAMP = "—";
export const INVALID_TIMESTAMP = "Некорректная дата";
