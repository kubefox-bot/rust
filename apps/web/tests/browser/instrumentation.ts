import type { Page } from "playwright";
import type { JobEventObservation } from "./types";
export const instrumentEvents = (page: Page) =>
  page.addInitScript(() => {
    const counters = { active: 0, created: 0 };
    const observations: JobEventObservation[] = [];
    Object.defineProperty(window, "__mailEventSources", { value: counters });
    Object.defineProperty(window, "__mailJobEvents", { value: observations });
    const record = (value: unknown): value is Record<string, unknown> =>
      typeof value === "object" && value !== null;
    const observe = (value: JobEventObservation) => {
      observations.push(value);
      if (observations.length > 32) observations.shift();
    };
    const Original = window.EventSource;
    window.EventSource = class extends Original {
      closed = false;
      constructor(url: string | URL, options?: EventSourceInit) {
        super(url, options);
        for (const event of ["open", "resync", "error"] as const) {
          this.addEventListener(event, () =>
            observe({ at: Date.now(), event }),
          );
        }
        this.addEventListener("snapshot", (event) => {
          if (
            !(event instanceof MessageEvent) ||
            typeof event.data !== "string"
          )
            return;
          try {
            const input: unknown = JSON.parse(event.data);
            const job = record(input) && record(input.job) ? input.job : null;
            const snapshot = job && record(job.snapshot) ? job.snapshot : null;
            observe({
              at: Date.now(),
              event: "snapshot",
              ...(job && typeof job.job_id === "string"
                ? { jobId: job.job_id }
                : {}),
              ...(job && typeof job.slot_held === "boolean"
                ? { slotHeld: job.slot_held }
                : {}),
              ...(snapshot && typeof snapshot.phase === "string"
                ? { phase: snapshot.phase }
                : {}),
              ...(record(input) && typeof input.revision === "string"
                ? { revision: input.revision }
                : {}),
            });
          } catch {
            observe({ at: Date.now(), event: "snapshot" });
          }
        });
        counters.active++;
        counters.created++;
      }
      override close() {
        if (!this.closed) {
          this.closed = true;
          counters.active--;
          observe({ at: Date.now(), event: "close" });
        }
        super.close();
      }
    };
  });
export const sourceCounts = (page: Page) =>
  page.evaluate(() => window.__mailEventSources);
