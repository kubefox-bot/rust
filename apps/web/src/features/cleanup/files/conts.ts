export const PAGE_SIZE = 50;
export const SEARCH_DEBOUNCE_MS = 250;
export const PROGRESS_REFRESH_MS = 1000;
export const MAX_SEARCH_BYTES = 512;

export const PAGE_SIZE_OPTIONS = [
  { value: 10, label: "10" },
  { value: 50, label: "50" },
  { value: 100, label: "100" },
];
export const FILES_ERROR_MESSAGES: Readonly<Record<number, string>> = {
  410: "Список файлов этого предпросмотра недоступен. Запустите новый предпросмотр.",
  404: "Задание не найдено. Обновите состояние сервера.",
  501: "Агент не поддерживает список файлов. Обновите агент, чтобы открыть подробности.",
};

export const EMPTY_FILTERS = { from: "", to: "", relation: "all" } as const;
export const RELATION_OPTIONS = [
  { value: "all", label: "Все" },
  { value: "linked", label: "Связан" },
  { value: "orphan", label: "Без записи" },
] as const;
