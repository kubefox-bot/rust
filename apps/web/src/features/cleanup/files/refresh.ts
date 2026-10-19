type Observation = Readonly<{
  revision: string;
  refreshVersion: number;
  terminal: boolean;
  visible: boolean;
}>;

export const createRefreshScheduler = ({
  refresh,
  onerror,
  delayMs,
}: Readonly<{
  refresh: () => Promise<void>;
  onerror: (cause: unknown) => void;
  delayMs: number;
}>) => {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let inFlight: object | null = null;
  let dirty: "none" | "delayed" | "immediate" = "none";
  let observed: Observation | null = null;
  const cancel = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    inFlight = null;
    dirty = "none";
  };
  const schedule = () => {
    if (inFlight !== null) {
      if (dirty === "none") dirty = "delayed";
      return;
    }
    if (timer !== null) return;
    timer = setTimeout(() => {
      timer = null;
      run();
    }, delayMs);
  };
  const run = () => {
    const owner = {};
    inFlight = owner;
    const settled = () => {
      if (inFlight !== owner) return;
      inFlight = null;
      const queued = dirty;
      dirty = "none";
      if (queued === "immediate") run();
      else if (queued === "delayed") schedule();
    };
    refresh().then(settled, (cause: unknown) => {
      if (inFlight !== owner) return;
      settled();
      onerror(cause);
    });
  };
  const flush = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    if (inFlight !== null) {
      dirty = "immediate";
      return;
    }
    run();
  };
  return {
    schedule,
    observe: (next: Observation) => {
      const previous = observed;
      observed = next;
      if (!next.visible || !previous?.visible) return;
      if (
        next.revision === previous.revision &&
        next.refreshVersion === previous.refreshVersion &&
        next.terminal === previous.terminal
      )
        return;
      if (next.terminal) flush();
      else schedule();
    },
    restart: () => {
      cancel();
      run();
    },
    cancel,
  };
};
