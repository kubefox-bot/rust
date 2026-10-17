import { subscribe } from "../../api";
import { EVENT_REFRESH_DELAY_MS } from "./conts";

export const observeHistory = (
  options: Readonly<{
    serverId: string;
    refresh: () => Promise<void>;
    onUnauthorized: () => Promise<void>;
    onError: (cause: unknown) => void;
  }>,
): (() => void) => {
  let closed = false;
  let running = false;
  let queued = false;
  let timer: ReturnType<typeof setTimeout> | undefined = undefined;
  let previous: Readonly<{ jobId: string; slotHeld: boolean }> | null = null;
  const schedule = () => {
    if (closed) return;
    queued = true;
    if (running || timer !== undefined) return;
    timer = setTimeout(() => {
      refresh().catch(options.onError);
    }, EVENT_REFRESH_DELAY_MS);
  };
  const refresh = async () => {
    timer = undefined;
    if (closed || !queued) return;
    queued = false;
    running = true;
    try {
      await options.refresh();
    } catch (cause) {
      if (!closed) options.onError(cause);
    } finally {
      running = false;
      if (queued) schedule();
    }
  };
  const release = subscribe({
    serverId: options.serverId,
    onEvent: ({ job }) => {
      if (closed) return;
      const newJob = previous?.jobId !== job.job_id;
      const settled = !job.slot_held && previous?.slotHeld !== false;
      previous = { jobId: job.job_id, slotHeld: job.slot_held };
      if (newJob || settled) schedule();
    },
    onResync: schedule,
    onUnauthorized: () => {
      if (!closed) options.onUnauthorized().catch(options.onError);
    },
  });
  return () => {
    closed = true;
    clearTimeout(timer);
    release();
  };
};
