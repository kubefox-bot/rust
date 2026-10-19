import { expect, test } from "bun:test";
import { startFixture } from "./fixture";
import { signIn } from "./cleanup-steps";
import { selectValue, selectDateTime } from "./controls";
import {
  instrumentGlobalListeners,
  globalListenerCounts,
} from "./global-listeners";

test("select owns global listeners only while open and preserves keyboard and outside dismissal", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await instrumentGlobalListeners(page);
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/#account`);
    const select = page.getByRole("combobox", {
      name: "Выход при бездействии",
      exact: true,
    });
    // Warm delegated option handlers before comparing the widget's own listeners.
    await select.click();
    await select.press("Escape");
    const closed = await globalListenerCounts(page);
    expect(closed.resize).toBe(0);
    expect(closed.scroll).toBe(0);
    await select.press("ArrowDown");
    await page
      .getByRole("listbox", { name: "Выход при бездействии", exact: true })
      .waitFor();
    expect(await globalListenerCounts(page)).toEqual({
      resize: closed.resize + 1,
      scroll: closed.scroll + 1,
      outside: closed.outside + 1,
    });
    await select.press("Escape");
    expect(await globalListenerCounts(page)).toEqual(closed);
    await select.click();
    await page.getByRole("heading", { name: "Сессия", exact: true }).click();
    expect(await select.getAttribute("aria-expanded")).toBe("false");
    expect(await globalListenerCounts(page)).toEqual(closed);
    await select.press("ArrowDown");
    await page.evaluate(() => {
      window.location.hash = "updates";
    });
    await page
      .getByLabel("Проверять обновления каждые, секунд", { exact: true })
      .waitFor();
    expect(await globalListenerCounts(page)).toEqual(closed);
  } finally {
    await fixture.close();
  }
}, 60000);

test("calendar releases viewport listeners on dismiss and unmount without changing fractional time", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await instrumentGlobalListeners(page);
    await signIn(page, fixture.origin);
    await selectValue(page, { label: "Период хранения", value: "before" });
    await selectDateTime(page, {
      label: "Граница по времени сервера",
      value: "2026-09-18T02:55:44.049364123",
    });
    const trigger = page.getByRole("button", {
      name: "Граница по времени сервера",
      exact: true,
    });
    await page.waitForFunction(
      () =>
        document
          .querySelector(".date-trigger")
          ?.getAttribute("aria-expanded") === "false",
    );
    const closed = await globalListenerCounts(page);
    expect(closed.resize).toBe(0);
    expect(closed.scroll).toBe(0);
    const original = await trigger.getAttribute("data-datetime-value");
    await trigger.click();
    const dialog = page.getByRole("dialog", {
      name: "Граница по времени сервера",
      exact: true,
    });
    await dialog
      .getByRole("spinbutton", { name: "Год календаря", exact: true })
      .waitFor();
    expect((await globalListenerCounts(page)).resize).toBe(closed.resize + 1);
    expect((await globalListenerCounts(page)).scroll).toBe(closed.scroll + 1);
    await page.setViewportSize({ width: 720, height: 700 });
    await dialog.getByRole("button", { name: "Отмена", exact: true }).click();
    await page.waitForFunction(
      () => window.__mailGlobalListeners?.().resize === 0,
    );
    expect(await globalListenerCounts(page)).toEqual(closed);
    expect(await trigger.getAttribute("data-datetime-value")).toBe(original);
    await trigger.click();
    await dialog.getByRole("button", { name: "Отмена", exact: true }).waitFor();
    await page.evaluate(() =>
      document.querySelector<HTMLButtonElement>("#retention-kind")?.click(),
    );
    await page
      .getByRole("option", { name: "Дней", exact: true })
      .evaluate((option) => {
        if (!(option instanceof HTMLButtonElement))
          throw new Error("Expected a retention option");
        option.click();
      });
    await page.waitForFunction(
      () => document.querySelector(".date-time-picker") === null,
    );
    expect((await globalListenerCounts(page)).resize).toBe(0);
    expect((await globalListenerCounts(page)).scroll).toBe(0);
  } finally {
    await fixture.close();
  }
}, 60000);
