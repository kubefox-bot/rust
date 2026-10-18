import type { BrowserContracts } from "../../api";
export type FileRow = BrowserContracts["preview_files_page"]["rows"][number];
export type FileSort = "path" | "size" | "created_at";
export type SortDirection = "asc" | "desc";
