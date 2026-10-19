import type { FileSort } from "./types";

export const SORTABLE_COLUMNS = [
  { id: "path", header: "Файл / путь" },
  { id: "size", header: "Размер" },
  { id: "created_at", header: "Создан" },
] as const satisfies ReadonlyArray<Readonly<{ id: FileSort; header: string }>>;
