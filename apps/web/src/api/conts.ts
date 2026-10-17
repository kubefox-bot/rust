export const API_PREFIX = "/api/v1";
export const UINT64_MAX = 18446744073709551615n;
export const CANONICAL_COUNTER = /^(0|[1-9][0-9]{0,19})$/;
export const CANONICAL_UUID =
  /^(?!00000000-0000-0000-0000-000000000000$)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
export const PARTIAL_DATE_TIME =
  /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?$/;
export const MAX_EVENT_BYTES = 4096;
export const MAX_RESPONSE_BYTES = 1024 * 1024;
export const REQUEST_TIMEOUT_MS = 20000;
export const UNSAFE_HEADERS = {
  "X-HMail-Request": "1",
  "Content-Type": "application/json",
} as const;
export const ERROR_MESSAGES: Readonly<Record<number, string>> = {
  400: "Проверьте параметры запроса.",
  401: "Требуется вход.",
  403: "Запрос отклонён проверкой безопасности.",
  404: "Запись не найдена.",
  409: "Состояние изменилось или другая операция уже выполняется.",
  429: "Операция занята. Повторите позже.",
  503: "Сервис или агент сейчас недоступен.",
};
