type Visibility = Pick<
  Document,
  "visibilityState" | "addEventListener" | "removeEventListener"
>;

export const mountVisibility = ({
  onVisible,
  onHidden,
  visibility = document,
}: Readonly<{
  onVisible: () => void;
  onHidden: () => void;
  visibility?: Visibility;
}>) => {
  let visible: boolean | null = null;
  const changed = () => {
    const next = visibility.visibilityState === "visible";
    if (next === visible) return;
    visible = next;
    if (next) onVisible();
    else onHidden();
  };
  visibility.addEventListener("visibilitychange", changed);
  changed();
  return () => visibility.removeEventListener("visibilitychange", changed);
};

export const mountVisiblePolling = ({
  refresh,
  intervalMs,
  visibility,
}: Readonly<{
  refresh: () => void;
  intervalMs: number;
  visibility?: Visibility;
}>) => {
  let timer: ReturnType<typeof setInterval> | null = null;
  const stop = () => {
    if (timer !== null) clearInterval(timer);
    timer = null;
  };
  const unwatch = mountVisibility({
    visibility,
    onVisible: () => {
      timer = setInterval(refresh, intervalMs);
      refresh();
    },
    onHidden: stop,
  });
  return () => {
    unwatch();
    stop();
  };
};
