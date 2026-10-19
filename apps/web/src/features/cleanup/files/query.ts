import type { FilesQuery } from "./types";
export const filesPath = ({
  jobId,
  query,
}: Readonly<{ jobId: string; query: FilesQuery }>) => {
  const parameters = new URLSearchParams({
    search: query.search,
    sort: query.sort,
    direction: query.direction,
    offset: String(query.offset),
    limit: String(query.limit),
  });
  if (query.createdFrom) parameters.set("created_from", query.createdFrom);
  if (query.createdTo) parameters.set("created_to", query.createdTo);
  if (query.orphan !== undefined)
    parameters.set("orphan", String(query.orphan));
  return `/jobs/${encodeURIComponent(jobId)}/files?${parameters}`;
};
export const clampOffset = ({
  offset,
  total,
  limit,
}: Readonly<{ offset: bigint; total: string; limit: number }>): bigint => {
  const count = BigInt(total);
  const pageSize = BigInt(limit);
  const last = count === 0n ? 0n : ((count - 1n) / pageSize) * pageSize;
  return offset > last ? last : offset;
};
