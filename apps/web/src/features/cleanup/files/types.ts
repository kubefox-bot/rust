import type { BrowserContracts } from "../../../api";
export type FilesPage = BrowserContracts["preview_files_page"];
export type FilesQuery = Readonly<{
  createdFrom?: string;
  createdTo?: string;
  orphan?: boolean;
  search: string;
  sort: "path" | "size" | "created_at";
  direction: "asc" | "desc";
  offset: bigint;
  limit: number;
}>;
export type FileFilters = Readonly<{
  from: string;
  to: string;
  relation: "all" | "linked" | "orphan";
}>;
