import type { BrowserContracts } from "../../api";
import type { PageRequest } from "./types";

export const PAGE_SIZE = 50;
export const EVENT_REFRESH_DELAY_MS = 150;
export const FIRST_PAGE: PageRequest = { cursor: null, previousCursors: [] };
export const OPERATION_LABELS = {
  preview: "Предпросмотр",
  clean: "Очистка",
} as const satisfies Record<
  BrowserContracts["job"]["intent"]["operation"],
  string
>;
export const SCAN_LABELS = {
  fast: "Быстрое сканирование",
  full: "Полное сканирование",
} as const satisfies Record<BrowserContracts["job"]["intent"]["scan"], string>;
