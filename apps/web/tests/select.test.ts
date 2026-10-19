import { expect, test } from "bun:test";
import { selectKey, matchingOption } from "../src/ui/select/keyboard";
import { menuPlacement } from "../src/ui/select/placement";

const closed = { open: false, activeIndex: 0, selectedIndex: 1, count: 3 };
const open = { ...closed, open: true, activeIndex: 2 };

test("opening keeps the selected build and Escape cancels a tentative choice", () => {
  expect(selectKey({ ...closed, key: "ArrowDown" })).toEqual({
    kind: "focus",
    index: 1,
  });
  expect(selectKey({ ...open, key: "Escape" })).toEqual({ kind: "close" });
  expect(selectKey({ ...open, key: "Enter" })).toEqual({
    kind: "commit",
    index: 2,
    preventDefault: true,
  });
  expect(selectKey({ ...open, key: "Tab" })).toEqual({
    kind: "commit",
    index: 2,
    preventDefault: false,
  });
});

test("keyboard navigation stays within available builds and empty lists cannot open", () => {
  expect(selectKey({ ...open, key: "ArrowDown" })).toEqual({
    kind: "focus",
    index: 2,
  });
  expect(selectKey({ ...open, activeIndex: 0, key: "ArrowUp" })).toEqual({
    kind: "focus",
    index: 0,
  });
  expect(selectKey({ ...open, key: "Home" })).toEqual({
    kind: "focus",
    index: 0,
  });
  expect(selectKey({ ...closed, key: "End" })).toEqual({
    kind: "focus",
    index: 2,
  });
  expect(selectKey({ ...closed, selectedIndex: -1, key: "Enter" })).toEqual({
    kind: "focus",
    index: 0,
  });
  expect(selectKey({ ...closed, count: 0, key: "Enter" })).toEqual({
    kind: "none",
  });
  expect(selectKey({ ...closed, key: "Tab" })).toEqual({ kind: "none" });
});

test("typing searches full build identifiers and wraps after the active build", () => {
  const options = ["0.1.0+first", "0.2.0+second", "0.1.0+third"].map(
    (value) => ({ value, label: value.split("+")[0] ?? value }),
  );
  expect(matchingOption({ options, query: "0.1", fromIndex: 0 })).toBe(2);
  expect(matchingOption({ options, query: "0.1", fromIndex: 2 })).toBe(0);
  expect(matchingOption({ options, query: "9", fromIndex: 0 })).toBeNull();
});

test("typing matches visible labels and numeric option values", () => {
  const options = [
    { value: 15, label: "15 минут" },
    { value: 60, label: "1 час" },
  ];
  expect(matchingOption({ options, query: "1 ч", fromIndex: 0 })).toBe(1);
  expect(matchingOption({ options, query: "60", fromIndex: 0 })).toBe(1);
  expect(
    matchingOption({
      options: [{ value: "key", label: "Закрытый ключ" }],
      query: "зак",
      fromIndex: 0,
    }),
  ).toBe(0);
});

test("version menus open above a low field and fit short viewports", () => {
  expect(menuPlacement({ top: 560, bottom: 610, viewportHeight: 720 })).toEqual(
    { side: "above", maxHeight: 280 },
  );
  expect(menuPlacement({ top: 100, bottom: 150, viewportHeight: 720 })).toEqual(
    { side: "below", maxHeight: 280 },
  );
  expect(menuPlacement({ top: 100, bottom: 150, viewportHeight: 220 })).toEqual(
    { side: "above", maxHeight: 76 },
  );
  expect(menuPlacement({ top: 0, bottom: 50, viewportHeight: 50 })).toEqual({
    side: "below",
    maxHeight: 0,
  });
});
