import type { BrowserContracts } from "../api";

export const createPackageCatalog = (
  request: () => Promise<BrowserContracts["packages"]>,
) => {
  let cached: Promise<BrowserContracts["packages"]> | null = null;
  return {
    read: (): Promise<BrowserContracts["packages"]> => {
      if (cached) return cached;
      cached = request().catch((error: unknown) => {
        cached = null;
        throw error;
      });
      return cached;
    },
  };
};
