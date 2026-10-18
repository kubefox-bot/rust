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
  let dirty = false;
  const cancel = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
    inFlight = null;
    dirty = false;
  };
  const schedule = () => {
    if (inFlight !== null) {
      dirty = true;
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
      if (!dirty) return;
      dirty = false;
      schedule();
    };
    refresh().then(settled, (cause: unknown) => {
      if (inFlight !== owner) return;
      settled();
      onerror(cause);
    });
  };
  return {
    schedule,
    restart: () => {
      cancel();
      run();
    },
    cancel,
  };
};
