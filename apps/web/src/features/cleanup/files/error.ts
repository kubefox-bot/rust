import { ApiError } from "../../../api";
import { FILES_ERROR_MESSAGES } from "./conts";
export const filesError = (cause: unknown): string => {
  if (cause instanceof ApiError)
    return FILES_ERROR_MESSAGES[cause.status] ?? cause.message;
  return cause instanceof Error
    ? cause.message
    : "Не удалось загрузить список файлов.";
};
