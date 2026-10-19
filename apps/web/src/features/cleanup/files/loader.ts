export const createPageLoader = <Page>(
  request: (signal: AbortSignal) => Promise<Page>,
) => {
  let owner: AbortController | null = null;
  return {
    cancel: () => {
      owner?.abort();
      owner = null;
    },
    load: async (): Promise<{ page: Page; current: () => boolean } | null> => {
      owner?.abort();
      const controller = new AbortController();
      owner = controller;
      const current = () => owner === controller && !controller.signal.aborted;
      try {
        const page = await request(controller.signal);
        return current() ? { page, current } : null;
      } catch (cause) {
        if (!current()) return null;
        throw cause;
      }
    },
  };
};
