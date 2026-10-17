import type { Page } from "playwright";
export const instrumentEvents = (page: Page) =>
  page.addInitScript(() => {
    const counters = { active: 0, created: 0 };
    Object.defineProperty(window, "__mailEventSources", { value: counters });
    const Original = window.EventSource;
    window.EventSource = class extends Original {
      closed = false;
      constructor(url: string | URL, options?: EventSourceInit) {
        super(url, options);
        counters.active++;
        counters.created++;
      }
      override close() {
        if (!this.closed) {
          this.closed = true;
          counters.active--;
        }
        super.close();
      }
    };
  });
export const sourceCounts = (page: Page) =>
  page.evaluate(() => window.__mailEventSources);
