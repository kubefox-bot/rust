import type { Context } from "./types";

export const createContextLoader = (
  request: (serverId: string, signal: AbortSignal) => Promise<Context>,
) => {
  let latest: object | null = null;
  return {
    invalidate: () => {
      latest = null;
    },
    load: async ({
      serverId,
      signal,
    }: Readonly<{ serverId: string; signal: AbortSignal }>) => {
      const token = {};
      latest = token;
      const current = () => latest === token && !signal.aborted;
      try {
        const context = await request(serverId, signal);
        return current() ? { context, current } : null;
      } catch (cause) {
        if (!current()) return null;
        throw cause;
      }
    },
  };
};
