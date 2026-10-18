import type { RunParameters } from "./types";
export const JOURNAL_LIMIT = 1000;
export const REFRESH_INTERVAL_MS = 15000;
export const DEFAULT_RETENTION_DAYS = 30;
export const DEFAULT_PARAMETERS: RunParameters = {
  scan: "fast",
  servicePolicy: "require_stopped",
  retention: { kind: "days", count: DEFAULT_RETENTION_DAYS },
  limits: { batch_size: 1000, workers: 2 },
};
export const JOURNAL_FOLLOW_DISTANCE_PX = 32;
export const MAX_BATCH_SIZE = 10000;
export const MAX_WORKERS = 8;
export const SCAN_OPTIONS = [
  { value: "fast", label: "По базе — быстро" },
  { value: "full", label: "Полная сверка файлов" },
] as const;
export const RETENTION_OPTIONS = [
  { value: "days", label: "Дней" },
  { value: "months", label: "Месяцев" },
  { value: "years", label: "Лет" },
  { value: "before", label: "До даты" },
] as const;
export const DATETIME_INPUT_FRACTION = /(\.\d{3})\d+$/;
export const DATETIME_INPUT_MINUTES = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/;
