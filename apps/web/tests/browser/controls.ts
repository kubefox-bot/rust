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
