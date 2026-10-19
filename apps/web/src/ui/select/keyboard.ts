import type { KeyAction, KeyInput, SelectOption } from "./types";
import { PAGE_STEP } from "./conts";

export const selectKey = ({
  key,
  open,
  activeIndex,
  selectedIndex,
  count,
}: KeyInput): KeyAction => {
  if (count === 0) return { kind: "none" };
  const current = Math.max(
    0,
    Math.min(count - 1, open ? activeIndex : selectedIndex),
  );
  switch (key) {
    case "ArrowDown":
      return {
        kind: "focus",
        index: open ? Math.min(count - 1, current + 1) : current,
      };
    case "ArrowUp":
      return {
        kind: "focus",
        index: open ? Math.max(0, current - 1) : current,
      };
    case "Home":
      return { kind: "focus", index: 0 };
    case "End":
      return { kind: "focus", index: count - 1 };
    case "PageDown":
      return { kind: "focus", index: Math.min(count - 1, current + PAGE_STEP) };
    case "PageUp":
      return { kind: "focus", index: Math.max(0, current - PAGE_STEP) };
    case "Enter":
    case " ":
      return open
        ? { kind: "commit", index: current, preventDefault: true }
        : { kind: "focus", index: current };
    case "Tab":
      return open
        ? { kind: "commit", index: current, preventDefault: false }
        : { kind: "none" };
    case "Escape":
      return open ? { kind: "close" } : { kind: "none" };
    default:
      return { kind: "none" };
  }
};

export const matchingOption = ({
  options,
  query,
  fromIndex,
}: Readonly<{
  options: readonly SelectOption<string | number>[];
  query: string;
  fromIndex: number;
}>): number | null => {
  if (!query) return null;
  const prefix = query.toLocaleLowerCase();
  for (let offset = 1; offset <= options.length; offset++) {
    const index = (fromIndex + offset + options.length) % options.length;
    const option = options[index];
    if (
      option &&
      (option.label.toLocaleLowerCase().startsWith(prefix) ||
        String(option.value).toLocaleLowerCase().startsWith(prefix))
    )
      return index;
  }
  return null;
};
