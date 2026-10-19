import { preview_files_page as previewFilesPageSchema } from "../../../api/contracts";
import { requestJson } from "../../../api";
import { filesPath } from "./query";
import type { FilesQuery } from "./types";
export const fetchFilesPage = ({
  jobId,
  query,
  signal,
}: Readonly<{ jobId: string; query: FilesQuery; signal: AbortSignal }>) =>
  requestJson({
    path: filesPath({ jobId, query }),
    responseSchema: previewFilesPageSchema,
    signal,
  });
