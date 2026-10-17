import type { BrowserContracts } from "../../api";
import type { RunParameters } from "./types";
export const JOURNAL_LIMIT = 1000;
export const REFRESH_INTERVAL_MS = 15000;
export const DEFAULT_RETENTION_DAYS = 30;
export const BYTE_UNITS = [
  ["ГиБ", 1073741824n],
  ["МиБ", 1048576n],
  ["КиБ", 1024n],
] as const;
export const DEFAULT_PARAMETERS: RunParameters = {
  scan: "fast",
  servicePolicy: "require_stopped",
  retention: { kind: "days", count: DEFAULT_RETENTION_DAYS },
  limits: { batch_size: 1000, workers: 2 },
};
export const PHASE_LABELS = {
  accepted: "Задание принято",
  running: "Выполняется",
  cancelling: "Отмена запрошена",
  restoring: "Восстановление службы",
  completed: "Завершено",
  partial: "Завершено с ошибками",
  failed: "Ошибка выполнения",
  cancelled: "Отменено",
  interrupted: "Прервано — нужна проверка",
} as const satisfies Record<BrowserContracts["phase"], string>;
export const JOURNAL_FOLLOW_DISTANCE_PX = 32;
export const MAX_BATCH_SIZE = 10000;
export const MAX_WORKERS = 8;
export const STAGE_LABELS = {
  preflight: "Проверка условий",
  snapshot: "Снимок данных",
  selection: "Поиск старой почты",
  files: "Удаление файлов",
  database: "Очистка базы",
  verification: "Проверка результата",
  directories: "Пустые каталоги",
  orphan_database: "Осиротевшие записи",
  references: "Проверка ссылок",
  scan: "Сканирование файлов",
  complete: "Готово",
  cancelled: "Отмена",
} as const satisfies Record<
  NonNullable<BrowserContracts["job"]["snapshot"]>["stage"] & string,
  string
>;
export const MAIL_LABELS = {
  unknown: "Неизвестно",
  running: "Работает",
  stopped: "Остановлена",
  restoring: "Восстанавливается",
} as const;
export const PUBLIC_ERROR_LABELS = {
  bad_request: "Некорректные параметры",
  unauthorized: "Требуется вход",
  busy: "Ресурс занят",
  conflict: "Состояние изменилось",
  unavailable: "Агент недоступен",
  interrupted: "Задание прервано",
  cancelled: "Задание отменено",
  cleanup_failed: "Ошибка очистки",
  restore_failed: "Не удалось восстановить службу",
  internal: "Внутренняя ошибка",
} as const satisfies Record<BrowserContracts["error"]["code"], string>;
