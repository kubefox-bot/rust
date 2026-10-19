import {
  server_id as serverIdSchema,
  counter as counterSchema,
  servers as serversSchema,
  history as historySchema,
} from "../../api/contracts";
import { ContractError, decode, requestJson } from "../../api";
import type { BrowserContracts } from "../../api";
import { PAGE_SIZE } from "./conts";

export const servers = (signal: AbortSignal) =>
  requestJson({ path: "/servers", responseSchema: serversSchema, signal });

export const history = async ({
  serverId,
  cursor,
  signal,
}: Readonly<{
  serverId: string;
  cursor: BrowserContracts["counter"] | null;
  signal: AbortSignal;
}>): Promise<BrowserContracts["history"]> => {
  const id = decode(serverIdSchema, serverId);
  const query = new URLSearchParams({ limit: String(PAGE_SIZE) });
  if (cursor !== null) query.set("cursor", decode(counterSchema, cursor));
  const page = await requestJson({
    path: `/servers/${id}/history?${query}`,
    responseSchema: historySchema,
    signal,
  });
  if (
    page.jobs.length > PAGE_SIZE ||
    page.jobs.some((job) => job.intent.server_id !== id)
  )
    throw new ContractError();
  return page;
};
