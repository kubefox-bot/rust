export const WEEKDAYS = ["Пн", "Вт", "Ср", "Чт", "Пт", "Сб", "Вс"];
export const GRID_DAYS = 42;
export const DAYS_PER_WEEK = 7;
export const DAY_MS = 86_400_000;
export const MIN_DAY = "0001-01-01";
export const MAX_DAY = "9999-12-31";
export const MIN_YEAR = 1;
export const MAX_YEAR = 9999;
export const TIME_PATTERN =
  /^([01]\d|2[0-3]):[0-5]\d(?::[0-5]\d(?:\.\d{1,9})?)?$/;
export const DAY_KEYS = {
  ArrowLeft: -1,
  ArrowRight: 1,
  ArrowUp: -7,
  ArrowDown: 7,
} as const;
export const POPOVER_WIDTH_PX = 336;
export const VIEWPORT_MARGIN_PX = 12;
export const POPOVER_GAP_PX = 8;
export const CALENDAR_LOCALE = "ru-RU";
export const TIME_BASIS = {
  server: {
    caption: "Дата и время сервера",
    hint: "Время почтового сервера, без пересчёта часового пояса.",
  },
  browser: {
    caption: "Дата и время браузера",
    hint: "Время в часовом поясе браузера.",
  },
} as const;
export const MONTH_OPTIONS = { month: "long", timeZone: "UTC" } as const;
export const DAY_OPTIONS = { dateStyle: "full", timeZone: "UTC" } as const;
