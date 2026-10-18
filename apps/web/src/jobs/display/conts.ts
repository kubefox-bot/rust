import type { BrowserContracts } from "../../api";
export const TERMINAL_PHASES = {
  accepted: false,
  running: false,
  cancelling: false,
  restoring: false,
  completed: true,
  partial: true,
  failed: true,
  cancelled: true,
  interrupted: true,
} as const satisfies Record<BrowserContracts["phase"], boolean>;
export const BYTE_UNITS = [
  ["Тб", 1099511627776n],
  ["Гб", 1073741824n],
  ["Мб", 1048576n],
  ["Кб", 1024n],
] as const;
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
