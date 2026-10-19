import {
  DATE_TIME_OPTIONS,
  INVALID_TIMESTAMP,
  MISSING_TIMESTAMP,
  SERVER_DATE_TIME_OPTIONS,
  TIME_LOCALE,
  UTC_TIME_OPTIONS,
} from "./conts";

type Timestamp = string | null | undefined;

const localDateTime = new Intl.DateTimeFormat(TIME_LOCALE, DATE_TIME_OPTIONS);
const serverDateTime = new Intl.DateTimeFormat(
  TIME_LOCALE,
  SERVER_DATE_TIME_OPTIONS,
);
const utcTime = new Intl.DateTimeFormat(TIME_LOCALE, UTC_TIME_OPTIONS);

const formatWith = ({
  value,
  formatter,
}: Readonly<{ value: Timestamp; formatter: Intl.DateTimeFormat }>): string => {
  if (value === null || value === undefined) return MISSING_TIMESTAMP;
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? INVALID_TIMESTAMP
    : formatter.format(date);
};

export const formatDateTime = (value: Timestamp): string =>
  formatWith({ value, formatter: localDateTime });

// Anchor server wall time to UTC only for display, without applying the browser's offset.
export const formatServerDateTime = (value: Timestamp): string =>
  formatWith({
    value: value === null || value === undefined ? value : `${value}Z`,
    formatter: serverDateTime,
  });

export const formatUtcTime = (value: Timestamp): string =>
  formatWith({ value, formatter: utcTime });
