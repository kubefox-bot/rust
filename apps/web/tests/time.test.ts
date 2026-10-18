import { expect, test } from "bun:test";
import {
  formatDateTime,
  formatServerDateTime,
  formatUtcTime,
} from "../src/lib/time";

test("server wall time stays unchanged across midnight and DST gaps", () => {
  expect(formatServerDateTime("2026-01-01T00:04:05.123456789")).toBe(
    "1 янв. 2026 г., 00:04:05",
  );
  expect(formatServerDateTime("2026-03-29T02:30:00")).toBe(
    "29 марта 2026 г., 02:30:00",
  );
});

test("absolute times respect offsets while journal times remain UTC", () => {
  expect(formatDateTime("2026-01-01T00:30:00+03:00")).toBe(
    formatDateTime("2025-12-31T21:30:00Z"),
  );
  expect(formatUtcTime("2026-01-01T00:30:00+03:00")).toBe("21:30:00");
  expect(formatUtcTime("2026-10-18T16:31:52.473227Z")).toBe("16:31:52");
});

test("missing timestamps and malformed timestamps have distinct display outcomes", () => {
  expect(formatDateTime(null)).toBe("—");
  expect(formatServerDateTime(undefined)).toBe("—");
  expect(formatDateTime("not-a-date")).toBe("Некорректная дата");
  expect(formatServerDateTime("not-a-date")).toBe("Некорректная дата");
  expect(formatUtcTime("")).toBe("Некорректная дата");
});
