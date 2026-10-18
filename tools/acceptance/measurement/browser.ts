import type { Page } from "playwright";
import { additionalScripts } from "./browser-assets";
import type { BrowserSnapshot } from "./types";

export const browserSnapshot = async (
  input: Readonly<{ name: string; page: Page }>,
): Promise<BrowserSnapshot> => {
  const session = await input.page.context().newCDPSession(input.page);
  try {
    const heap = await session.send("Runtime.getHeapUsage");
    const dom = await session.send("Memory.getDOMCounters");
    const content = await input.page.evaluate(() => ({
      domElements: document.querySelectorAll("*").length,
      journalDomRows: document.querySelectorAll('[role="log"] .log-entry')
        .length,
      historyDomRows: document.querySelectorAll("tbody tr").length,
      scripts: performance
        .getEntriesByType("resource")
        .filter(
          (entry): entry is PerformanceResourceTiming =>
            entry instanceof PerformanceResourceTiming &&
            new URL(entry.name).pathname.endsWith(".js"),
        )
        .map((entry) => ({
          path: new URL(entry.name).pathname,
          encodedBodyBytes: entry.encodedBodySize,
          decodedBodyBytes: entry.decodedBodySize,
          transferBytes: entry.transferSize,
        })),
    }));
    return {
      name: input.name,
      ...content,
      domNodes: dom.nodes,
      jsHeapUsedBytes: heap.usedSize,
      jsHeapTotalBytes: heap.totalSize,
    };
  } finally {
    await session.detach();
  }
};
export const verifyLazyPages = (
  input: Readonly<{
    cleanup: BrowserSnapshot;
    history: BrowserSnapshot;
    settings: BrowserSnapshot;
    initialRequests: readonly string[];
  }>,
) => {
  const initial = input.cleanup.scripts.map((script) => script.path);
  const history = additionalScripts({
    initial,
    selected: input.history.scripts.map((script) => script.path),
  });
  const settings = additionalScripts({
    initial,
    selected: input.settings.scripts.map((script) => script.path),
  });
  if (history.length === 0 || settings.length === 0)
    throw new Error(
      "Selected lazy pages did not load distinct JavaScript chunks",
    );
  const prematureRequests = input.initialRequests.filter(
    (path) =>
      path.endsWith("/history") ||
      path.includes("/profile") ||
      path.includes("/settings/session"),
  );
  if (prematureRequests.length > 0)
    throw new Error("Cleanup eagerly requested history/settings data");
  return {
    historyAdditionalScripts: history,
    settingsAdditionalScripts: settings,
    prematureDataRequests: prematureRequests,
  };
};
