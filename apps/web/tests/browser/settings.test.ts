import { selectValue } from "./controls";
import { expect, test } from "bun:test";
import { startFixture } from "./fixture";
import { signIn } from "./cleanup-steps";
import { decode } from "../../src/api";
test("profile edits preserve the stored password with Keep and unconfigured agents are not ready", async () => {
  const f = await startFixture();
  try {
    const page = await f.context.newPage();
    await signIn(page, f.origin);
    await page.goto(`${f.origin}/settings/?server=${f.servers[0].id}`);
    await page
      .getByLabel("Хост MySQL", { exact: true })
      .waitFor({ timeout: 3000 });
    expect(
      await page.getByLabel("Пароль MySQL", { exact: true }).inputValue(),
    ).toBe("");
    const saved = page.waitForRequest(
      (request) =>
        request.method() === "PATCH" &&
        new URL(request.url()).pathname.endsWith("/profile"),
    );
    await page
      .getByLabel("Каталог почты", { exact: true })
      .fill("C:\\Mail Data\\Новая папка");
    await page
      .getByRole("button", { name: "Сохранить профиль", exact: true })
      .click();
    const input = decode("save_profile", (await saved).postDataJSON());
    expect(input.profile.password.kind).toBe("keep");
    await page.getByText("Профиль сохранён", { exact: true }).waitFor();
    await selectValue(page, { label: "Сервер", value: f.servers[1].id });
    await page
      .getByText("Требуется настройка", { exact: true })
      .first()
      .waitFor();
  } finally {
    await f.close();
  }
}, 120000);
test("replacement password is sent only to the selected server and cleared after saving", async () => {
  const f = await startFixture();
  try {
    const page = await f.context.newPage();
    await signIn(page, f.origin);
    await page.goto(`${f.origin}/settings/?server=${f.servers[0].id}`);
    await page.getByLabel("Заменить пароль MySQL", { exact: true }).check();
    await page
      .getByLabel("Пароль MySQL", { exact: true })
      .fill("replacement-fixture-password");
    const saved = page.waitForRequest(
      (request) =>
        request.method() === "PATCH" &&
        new URL(request.url()).pathname.endsWith("/profile"),
    );
    await page
      .getByRole("button", { name: "Сохранить профиль", exact: true })
      .click();
    const request = await saved;
    const input = decode("save_profile", request.postDataJSON());
    expect(new URL(request.url()).pathname).toBe(
      `/api/v1/servers/${f.servers[0].id}/profile`,
    );
    expect(input.profile.password).toEqual({
      kind: "replace",
      value: "replacement-fixture-password",
    });
    await page.getByText("Профиль сохранён", { exact: true }).waitFor();
    expect(
      await page.getByLabel("Пароль MySQL", { exact: true }).inputValue(),
    ).toBe("");
    expect(
      await page.evaluate(
        () =>
          JSON.stringify({ ...localStorage }) +
          JSON.stringify({ ...sessionStorage }),
      ),
    ).not.toContain("replacement-fixture-password");
    await selectValue(page, { label: "Сервер", value: f.servers[1].id });
    await page
      .getByText("Требуется настройка", { exact: true })
      .first()
      .waitFor();
    expect(
      await page.getByLabel("Пароль MySQL", { exact: true }).inputValue(),
    ).toBe("");
  } finally {
    await f.close();
  }
}, 120000);
test("an explicit missing server never silently opens another technical profile", async () => {
  const f = await startFixture();
  try {
    const page = await f.context.newPage();
    await signIn(page, f.origin);
    const paths: string[] = [];
    page.on("request", (request) =>
      paths.push(new URL(request.url()).pathname),
    );
    await page.goto(
      `${f.origin}/settings/?server=11111111-1111-4111-8111-111111111111`,
    );
    await page
      .getByRole("alert")
      .filter({ hasText: "Выбранный сервер не найден" })
      .waitFor();
    expect(paths.filter((path) => path.endsWith("/profile"))).toHaveLength(0);
    expect(await page.getByLabel("Хост MySQL", { exact: true }).count()).toBe(
      0,
    );
  } finally {
    await f.close();
  }
}, 120000);
