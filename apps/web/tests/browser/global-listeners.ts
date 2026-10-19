import type { Page } from "playwright";
import type { GlobalListenerCounts } from "./types";

export const instrumentGlobalListeners = (page: Page) =>
  page.addInitScript(() => {
    const add = EventTarget.prototype.addEventListener;
    const remove = EventTarget.prototype.removeEventListener;
    const active: {
      target: EventTarget;
      type: string;
      listener: EventListenerOrEventListenerObject;
      capture: boolean;
    }[] = [];
    const tracked = (target: EventTarget, type: string) =>
      (target === window && (type === "resize" || type === "scroll")) ||
      (target === document && type === "pointerdown");
    EventTarget.prototype.addEventListener = function (
      type,
      listener,
      options,
    ) {
      add.call(this, type, listener, options);
      if (!listener || !tracked(this, type)) return;
      const capture =
        typeof options === "boolean" ? options : (options?.capture ?? false);
      if (
        !active.some(
          (entry) =>
            entry.target === this &&
            entry.type === type &&
            entry.listener === listener &&
            entry.capture === capture,
        )
      )
        active.push({ target: this, type, listener, capture });
    };
    EventTarget.prototype.removeEventListener = function (
      type,
      listener,
      options,
    ) {
      remove.call(this, type, listener, options);
      const capture =
        typeof options === "boolean" ? options : (options?.capture ?? false);
      const index = active.findIndex(
        (entry) =>
          entry.target === this &&
          entry.type === type &&
          entry.listener === listener &&
          entry.capture === capture,
      );
      if (index !== -1) active.splice(index, 1);
    };
    Object.defineProperty(window, "__mailGlobalListeners", {
      value: () => ({
        resize: active.filter(
          (entry) => entry.target === window && entry.type === "resize",
        ).length,
        scroll: active.filter(
          (entry) => entry.target === window && entry.type === "scroll",
        ).length,
        outside: active.filter(
          (entry) => entry.target === document && entry.type === "pointerdown",
        ).length,
      }),
    });
  });

export const globalListenerCounts = (
  page: Page,
): Promise<GlobalListenerCounts> =>
  page.evaluate(() => {
    if (!window.__mailGlobalListeners)
      throw new Error("Global listener instrumentation is missing");
    return window.__mailGlobalListeners();
  });
