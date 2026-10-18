import {
  DAY_KEYS,
  DAY_MS,
  DAYS_PER_WEEK,
  GRID_DAYS,
  MAX_DAY,
  MIN_DAY,
  TIME_PATTERN,
} from "./conts";

const dateOf = (day: string): Date => new Date(`${day}T00:00:00Z`);
const dayOf = (date: Date): string => date.toISOString().slice(0, 10);
const bounded = (date: Date): Date =>
  new Date(
    Math.max(
      dateOf(MIN_DAY).getTime(),
      Math.min(dateOf(MAX_DAY).getTime(), date.getTime()),
    ),
  );

export const moveDay = ({
  day,
  days,
}: Readonly<{ day: string; days: number }>): string =>
  dayOf(bounded(new Date(dateOf(day).getTime() + days * DAY_MS)));

export const shiftMonth = ({
  day,
  months,
}: Readonly<{ day: string; months: number }>): string => {
  const date = dateOf(day);
  const target = dateOf(day);
  target.setUTCFullYear(
    date.getUTCFullYear(),
    date.getUTCMonth() + months + 1,
    0,
  );
  target.setUTCDate(Math.min(date.getUTCDate(), target.getUTCDate()));
  return dayOf(bounded(target));
};

export const monthDays = (day: string): ReadonlyArray<string | null> => {
  const first = `${day.slice(0, 7)}-01`;
  const offset =
    (dateOf(first).getUTCDay() + DAYS_PER_WEEK - 1) % DAYS_PER_WEEK;
  return Array.from({ length: GRID_DAYS }, (_, index) => {
    const date = new Date(dateOf(first).getTime() + (index - offset) * DAY_MS);
    if (
      date.getTime() < dateOf(MIN_DAY).getTime() ||
      date.getTime() > dateOf(MAX_DAY).getTime()
    )
      return null;
    return dayOf(date);
  });
};

export const replaceDay = ({
  value,
  day,
}: Readonly<{ value: string; day: string }>): string =>
  `${day}T${value.split("T")[1] || "00:00:00"}`;

export const replaceTime = ({
  value,
  time,
}: Readonly<{ value: string; time: string }>): string | null => {
  if (!TIME_PATTERN.test(time) || !value) return null;
  const canonical = time.length === 5 ? `${time}:00` : time;
  return `${value.slice(0, 10)}T${canonical}`;
};

export const calendarKey = ({
  day,
  key,
  shift = false,
}: Readonly<{ day: string; key: string; shift?: boolean }>): string | null => {
  if (Object.hasOwn(DAY_KEYS, key))
    return moveDay({ day, days: DAY_KEYS[key as keyof typeof DAY_KEYS] });
  const weekday = (dateOf(day).getUTCDay() + DAYS_PER_WEEK - 1) % DAYS_PER_WEEK;
  switch (key) {
    case "PageUp":
      return shiftMonth({ day, months: shift ? -12 : -1 });
    case "PageDown":
      return shiftMonth({ day, months: shift ? 12 : 1 });
    case "Home":
      return moveDay({ day, days: -weekday });
    case "End":
      return moveDay({ day, days: DAYS_PER_WEEK - 1 - weekday });
    default:
      return null;
  }
};
