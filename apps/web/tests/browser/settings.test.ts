import { save_profile as saveProfileSchema } from "../../src/api/contracts";
import { selectValue } from "./controls";
import { expect, test } from "bun:test";
import { startFixture } from "./fixture";
import { signIn } from "./cleanup-steps";
import { decode } from "../../src/api";
test("agent settings and footer share the page package catalog", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    const packages: string[] = [];
    page.on("request", (request) => {
      const path = new URL(request.url()).pathname;
      if (path === "/api/v1/enrollment/packages") packages.push(path);
    });
    await page.goto(`${fixture.origin}/settings/#agents`);
    await page
      .getByRole("heading", { name: "Добавить сервер", exact: true })
      .waitFor();
    expect(packages).toHaveLength(1);
    await page.getByRole("button", { name: "Обновления", exact: true }).click();
    await page
      .getByLabel("Проверять обновления каждые, секунд", { exact: true })
      .waitFor();
    await page.getByRole("button", { name: "Агенты", exact: true }).click();
    await page
      .getByRole("heading", { name: "Добавить сервер", exact: true })
      .waitFor();
    expect(packages).toHaveLength(1);
  } finally {
    await fixture.close();
  }
}, 60000);
test("center settings do not load mail context or profile", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    const mailRequests: string[] = [];
    page.on("request", (request) => {
      const path = new URL(request.url()).pathname;
      if (path.endsWith("/cleanup-context") || path.endsWith("/profile"))
        mailRequests.push(path);
    });
    await page.goto(`${fixture.origin}/settings/#updates`);
    await page
      .getByLabel("Проверять обновления каждые, секунд", { exact: true })
      .waitFor();
    expect(mailRequests).toEqual([]);
    await page
      .getByRole("button", { name: "Учётная запись", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Ключ восстановления", exact: true })
      .waitFor();
    expect(mailRequests).toEqual([]);
  } finally {
    await fixture.close();
  }
}, 60000);
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
    const input = decode(saveProfileSchema, (await saved).postDataJSON());
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
    const input = decode(saveProfileSchema, request.postDataJSON());
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

test("center update policy is configurable without a selected server and persists after reload", async () => {
  const f = await startFixture();
  try {
    const page = await f.context.newPage();
    await signIn(page, f.origin);
    await page.goto(`${f.origin}/settings/#updates`);
    const interval = page.getByLabel("Проверять обновления каждые, секунд", {
      exact: true,
    });
    await interval.waitFor({ timeout: 3000 });
    expect(await interval.inputValue()).toBe("60");
    await interval.fill("120");
    await page
      .getByLabel("Ожидать завершения работы, минут", { exact: true })
      .fill("30");
    await page
      .getByRole("button", {
        name: "Сохранить настройки обновлений",
        exact: true,
      })
      .click();
    await page
      .getByText("Настройки обновлений сохранены", { exact: true })
      .waitFor();
    await page.reload();
    await page.waitForFunction(
      () =>
        document.querySelector<HTMLInputElement>("#update-check-interval")
          ?.value === "120",
    );
    expect(
      await page
        .getByLabel("Ожидать завершения работы, минут", { exact: true })
        .inputValue(),
    ).toBe("30");
    expect(await interval.getAttribute("min")).toBe("30");
    expect(await interval.getAttribute("max")).toBe("86400");
  } finally {
    await f.close();
  }
}, 120000);

test("failed update policy save preserves the draft and never claims success", async () => {
  const f = await startFixture();
  try {
    const page = await f.context.newPage();
    await signIn(page, f.origin);
    await page.goto(`${f.origin}/settings/#updates`);
    const interval = page.getByLabel("Проверять обновления каждые, секунд", {
      exact: true,
    });
    await interval.waitFor({ timeout: 3000 });
    await interval.fill("300");
    await page.route("**/api/v1/settings/updates", async (route) => {
      if (route.request().method() === "PATCH")
        await route.fulfill({ status: 503, body: "" });
      else await route.continue();
    });
    await page
      .getByRole("button", {
        name: "Сохранить настройки обновлений",
        exact: true,
      })
      .click();
    await page.getByRole("alert").waitFor();
    expect(await interval.inputValue()).toBe("300");
    expect(
      await page
        .getByText("Настройки обновлений сохранены", { exact: true })
        .count(),
    ).toBe(0);
    await page.unroute("**/api/v1/settings/updates");
    await page
      .getByRole("button", {
        name: "Сохранить настройки обновлений",
        exact: true,
      })
      .click();
    await page
      .getByText("Настройки обновлений сохранены", { exact: true })
      .waitFor();
    await page.reload();
    await page.waitForFunction(
      () =>
        document.querySelector<HTMLInputElement>("#update-check-interval")
          ?.value === "300",
    );
  } finally {
    await f.close();
  }
}, 120000);
