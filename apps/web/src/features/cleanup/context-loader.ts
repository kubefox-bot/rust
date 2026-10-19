import type { Context } from "./types";

type Owner = Readonly<{
  serverId: string;
  signal: AbortSignal;
  controller: AbortController;
}>;
type Loaded = Readonly<{ context: Context; current: () => boolean }> | null;

export const createContextLoader = (
  request: (serverId: string, signal: AbortSignal) => Promise<Context>,
) => {
  let latest: Owner | null = null;
  let pending: Promise<Loaded> | null = null;
  const invalidate = () => {
    latest?.controller.abort();
    latest = null;
    pending = null;
  };
  return {
    invalidate,
    load: ({
      serverId,
      signal,
    }: Readonly<{
      serverId: string;
      signal: AbortSignal;
    }>): Promise<Loaded> => {
      if (signal.aborted) return Promise.resolve(null);
      if (pending && latest?.serverId === serverId && latest.signal === signal)
        return pending;
      invalidate();
      const controller = new AbortController();
      const owner = { serverId, signal, controller };
      const requestSignal = AbortSignal.any([signal, controller.signal]);
      latest = owner;
      const current = () => latest === owner && !requestSignal.aborted;
      const load = async () => {
        try {
          const context = await request(serverId, requestSignal);
          return current() ? { context, current } : null;
        } catch (cause) {
          if (!current()) return null;
          throw cause;
        }
      };
      pending = load().finally(() => {
        if (latest === owner) pending = null;
      });
      return pending;
    },
  };
};
