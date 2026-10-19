import { expect, test } from "bun:test";
import {
  calendarKey,
  monthDays,
  shiftMonth,
  replaceDay,
  replaceTime,
} from "../src/ui/date-time/calendar";

test("calendar starts on Monday and includes leap day without browser timezone shifts", () => {
  const days = monthDays("2024-02-15");
  expect(days[0]).toBe("2024-01-29");
  expect(days).toContain("2024-02-29");
  expect(days.at(-1)).toBe("2024-03-10");
  expect(shiftMonth({ day: "2024-01-31", months: 1 })).toBe("2024-02-29");
  expect(shiftMonth({ day: "2024-12-31", months: 1 })).toBe("2025-01-31");
});

test("changing calendar day preserves server wall time and full fractional precision", () => {
  expect(
    replaceDay({ value: "2026-09-18T02:55:44.049364123", day: "2026-10-25" }),
  ).toBe("2026-10-25T02:55:44.049364123");
  expect(replaceDay({ value: "", day: "2026-10-25" })).toBe(
    "2026-10-25T00:00:00",
  );
  expect(
    replaceTime({ value: "2026-10-25T02:55:44.049364123", time: "01:30" }),
  ).toBe("2026-10-25T01:30:00");
  expect(
    replaceTime({
      value: "2026-10-25T02:55:44.049364123",
      time: "01:30:22.123456789",
    }),
  ).toBe("2026-10-25T01:30:22.123456789");
  expect(
    replaceTime({ value: "2026-10-25T02:55:44", time: "25:00" }),
  ).toBeNull();
  expect(
    replaceTime({ value: "2026-10-25T02:55:44", time: "12:99" }),
  ).toBeNull();
});

test("month navigation preserves early years and stays within supported date bounds", () => {
  expect(shiftMonth({ day: "0001-01-31", months: 1 })).toBe("0001-02-28");
  expect(shiftMonth({ day: "0001-01-01", months: -1 })).toBe("0001-01-01");
  expect(shiftMonth({ day: "9999-12-31", months: 1 })).toBe("9999-12-31");
});

test("calendar keyboard crosses months and selects Monday-based week boundaries", () => {
  expect(calendarKey({ day: "2024-02-29", key: "ArrowRight" })).toBe(
    "2024-03-01",
  );
  expect(calendarKey({ day: "2024-03-03", key: "Home" })).toBe("2024-02-26");
  expect(calendarKey({ day: "2024-02-29", key: "End" })).toBe("2024-03-03");
  expect(calendarKey({ day: "2024-02-29", key: "PageDown", shift: true })).toBe(
    "2025-02-28",
  );
  expect(calendarKey({ day: "2024-02-29", key: "Escape" })).toBeNull();
});
