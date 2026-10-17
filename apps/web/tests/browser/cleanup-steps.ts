import type { Page } from "playwright";
import type { startFixture } from "./fixture";
export const signIn = async (page: Page, origin: string) => {
  await page.goto(`${origin}/login/`);
  await page.getByLabel("Логин", { exact: true }).fill("admin");
  await page
    .getByLabel("Пароль", { exact: true })
    .fill("fixture-center-password");
  await page
    .getByRole("button", { name: "Войти в центр", exact: true })
    .click();
  await page.getByLabel("Сервер", { exact: true }).waitFor();
};
export const completedPreview = async (
  page: Page,
  fixture: Awaited<ReturnType<typeof startFixture>>,
  server: string,
) => {
  await page.getByLabel("Сервер", { exact: true }).selectOption(server);
  await page.getByRole("button", { name: "Предпросмотр", exact: true }).click();
  await page.getByText("Выполняется", { exact: true }).waitFor();
  await fixture.finishLatest(server);
  await page
    .getByRole("button", { name: "Очистить", exact: true })
    .click({ trial: true });
};
