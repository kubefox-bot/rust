import type { Page } from "playwright";

export const selectValue = async (
  page: Page,
  { label, value }: Readonly<{ label: string; value: string | number }>,
) => {
  await page.getByRole("combobox", { name: label, exact: true }).click();
  await page
    .getByRole("listbox", { name: label, exact: true })
    .locator(`[data-value="${value}"]`)
    .click();
};

export const selectDateTime = async (
  page: Page,
  { label, value }: Readonly<{ label: string; value: string }>,
) => {
  await page.getByRole("button", { name: label, exact: true }).click();
  const picker = page.getByRole("dialog", { name: label, exact: true });
  await picker
    .getByRole("spinbutton", { name: "Год календаря" })
    .fill(value.slice(0, 4));
  await picker.getByRole("spinbutton", { name: "Год календаря" }).blur();
  const current = await picker
    .locator("[data-calendar-month]")
    .getAttribute("data-calendar-month");
  if (!current) throw new Error("Calendar month is unavailable");
  const months =
    Number(value.slice(0, 4)) * 12 +
    Number(value.slice(5, 7)) -
    Number(current.slice(0, 4)) * 12 -
    Number(current.slice(5, 7));
  for (let step = 0; step < Math.abs(months); step++)
    await picker
      .getByRole("button", {
        name: months > 0 ? "Следующий месяц" : "Предыдущий месяц",
      })
      .click();
  await picker.locator(`[data-calendar-day="${value.slice(0, 10)}"]`).click();
  await picker
    .getByRole("textbox", { name: "Время", exact: true })
    .fill(value.split("T")[1] ?? "00:00:00");
  await picker.getByRole("button", { name: "Применить", exact: true }).click();
};
