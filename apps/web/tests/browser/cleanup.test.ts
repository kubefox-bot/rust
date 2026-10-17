import { expect, test } from "bun:test";
import { startFixture } from "./fixture";

test("login, exact-target preview and confirmation work through Rust HTTPS", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await page.goto(`${fixture.origin}/login/`);
    await page.getByLabel("Логин").fill("admin");
    await page
      .getByLabel("Пароль", { exact: true })
      .fill("fixture-center-password");
    await page
      .getByRole("button", { name: "Войти в центр", exact: true })
      .click();
    await page
      .getByLabel("Сервер", { exact: true })
      .selectOption(fixture.servers[0].id);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page.getByText("Выполняется", { exact: true }).waitFor();
    await fixture.finishLatest(fixture.servers[0].id);
    await page
      .getByRole("button", { name: "Очистить", exact: true })
      .waitFor({ state: "visible" });
    await page
      .getByRole("button", { name: "Очистить", exact: true })
      .click({ trial: true });
    await page.getByRole("button", { name: "Очистить", exact: true }).click();
    await page
      .getByRole("dialog")
      .getByText(fixture.servers[0].name, { exact: true })
      .waitFor();
    await page.getByRole("button", { name: "Отмена", exact: true }).click();
    const stats = await fixture.stats();
    expect(stats[0].puts).toBe(1);
  } finally {
    await fixture.close();
  }
}, 60000);

test("two tabs keep confirmation target; managed cancellation restores before final state and logout never cancels", async () => {
  const { signIn, completedPreview } = await import("./cleanup-steps");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page
      .getByLabel("Останавливать службу на время очистки", { exact: false })
      .check();
    await completedPreview(page, fixture, fixture.servers[0].id);
    await page.getByRole("button", { name: "Очистить", exact: true }).click();
    const other = await fixture.context.newPage();
    await other.goto(`${fixture.origin}/?server=${fixture.servers[1].id}`);
    await other.getByLabel("Сервер", { exact: true }).waitFor();
    expect(await other.getByLabel("Сервер", { exact: true }).inputValue()).toBe(
      fixture.servers[1].id,
    );
    await page
      .getByRole("dialog")
      .getByText(fixture.servers[0].name, { exact: true })
      .waitFor();
    await page
      .getByLabel(
        "Подтверждаю удаление и временную остановку почтовой службы.",
        { exact: true },
      )
      .check();
    await page
      .getByRole("button", { name: "Подтвердить очистку", exact: true })
      .click();
    await page.getByText("Выполняется", { exact: true }).waitFor();
    const before = await fixture.stats();
    expect(
      before.find((value) => value.id === fixture.servers[0].id)?.puts,
    ).toBe(2);
    expect(
      before.find((value) => value.id === fixture.servers[1].id)?.puts,
    ).toBe(0);
    await page
      .getByRole("button", { name: "Отменить задание", exact: true })
      .click();
    await page.getByText("Восстановление службы", { exact: true }).waitFor();
    expect(
      await page
        .getByRole("button", { name: "Предпросмотр", exact: true })
        .isDisabled(),
    ).toBe(true);
    await page.getByRole("button", { name: "Выйти", exact: true }).click();
    await page.getByLabel("Логин", { exact: true }).waitFor();
    const after = await fixture.stats();
    expect(
      after.find((value) => value.id === fixture.servers[0].id)?.cancels,
    ).toBe(1);
    await fixture.phase(fixture.servers[0].id, "cancelled");
    await signIn(page, fixture.origin);
    await page
      .getByRole("heading", { name: "Отменено", exact: true })
      .waitFor();
    expect(
      (await fixture.stats()).find(
        (value) => value.id === fixture.servers[0].id,
      )?.puts,
    ).toBe(2);
  } finally {
    await fixture.close();
  }
}, 120000);

test("full preview while mail runs sends no job; fullscreen keeps one SSE and footer at every width", async () => {
  const { signIn } = await import("./cleanup-steps");
  const { instrumentEvents, sourceCounts } = await import("./instrumentation");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await instrumentEvents(page);
    await signIn(page, fixture.origin);
    await page
      .getByLabel("Режим сканирования", { exact: true })
      .selectOption("full");
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page
      .getByRole("alert")
      .getByText(/Для полного предпросмотра/)
      .waitFor();
    expect(
      (await fixture.stats()).every(
        (value) => value.puts === 0 && value.cancels === 0,
      ),
    ).toBe(true);
    await page
      .getByLabel("Режим сканирования", { exact: true })
      .selectOption("fast");
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page.getByText("Выполняется", { exact: true }).waitFor();
    const id = await page
      .getByRole("region", { name: "Состояние задания" })
      .locator("code")
      .textContent();
    for (const width of [1440, 1024, 736, 375, 320]) {
      await page.setViewportSize({ width, height: 900 });
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
      await page
        .getByRole("button", { name: "Развернуть журнал", exact: true })
        .click();
      await page
        .getByRole("button", { name: "Свернуть журнал", exact: true })
        .waitFor();
      expect(await sourceCounts(page)).toEqual({ active: 1, created: 1 });
      expect(await page.getByRole("contentinfo").isVisible()).toBe(true);
      const footer = await page.getByRole("contentinfo").boundingBox();
      expect(footer !== null && footer.y + footer.height <= 901).toBe(true);
      expect(
        await page
          .getByRole("region", { name: "Состояние задания" })
          .locator("code")
          .textContent(),
      ).toBe(id);
      await page.keyboard.press("Escape");
      expect(
        await page
          .getByRole("button", { name: "Развернуть журнал", exact: true })
          .evaluate((element) => element === document.activeElement),
      ).toBe(true);
    }
    await page.context().setOffline(true);
    await page.context().setOffline(false);
    await page.getByRole("button", { name: "Обновить состояние" }).click();
    expect(await sourceCounts(page)).toEqual({ active: 1, created: 1 });
  } finally {
    await fixture.close();
  }
}, 120000);

test("HTTPS trust is limited to the fixture CA rather than all certificates", async () => {
  const fixture = await startFixture();
  try {
    const other = await startFixture();
    try {
      const page = await fixture.context.newPage();
      const failure = await page
        .goto(other.origin)
        .catch((error: unknown) => error);
      expect(failure).toBeInstanceOf(Error);
      if (!(failure instanceof Error))
        throw new Error("Untrusted certificate was accepted");
      expect(failure.message).toContain("ERR_CERT_AUTHORITY_INVALID");
    } finally {
      await other.close();
    }
  } finally {
    await fixture.close();
  }
}, 120000);

test("Back navigation after logout never restores an authorized screen or sends cancellation", async () => {
  const { signIn } = await import("./cleanup-steps");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Выполняется", exact: true })
      .waitFor();
    await page.goto(`${fixture.origin}/?server=${fixture.servers[1].id}`);
    await page.getByLabel("Сервер", { exact: true }).waitFor();
    await page.getByRole("button", { name: "Выйти", exact: true }).click();
    await page.getByLabel("Логин", { exact: true }).waitFor();
    await page.goBack();
    await page
      .getByRole("link", { name: "Войти в центр", exact: true })
      .waitFor();
    expect(
      await page.getByRole("region", { name: "Состояние задания" }).count(),
    ).toBe(0);
    expect((await fixture.stats()).every((value) => value.cancels === 0)).toBe(
      true,
    );
  } finally {
    await fixture.close();
  }
}, 120000);

test("unknown delivery offers reconciliation and cancellation without a blind second dispatch", async () => {
  const { signIn } = await import("./cleanup-steps");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await fixture.loseResponse(fixture.servers[0].id, true);
    await fixture.hideLookup(fixture.servers[0].id, true);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Проверка доставки задания", exact: true })
      .waitFor();
    expect(
      await page
        .getByRole("button", { name: "Очистить", exact: true })
        .isDisabled(),
    ).toBe(true);
    expect(
      await page
        .getByRole("button", { name: "Предпросмотр", exact: true })
        .isDisabled(),
    ).toBe(true);
    await page
      .getByRole("button", { name: "Проверить состояние", exact: true })
      .click();
    expect(
      (await fixture.stats()).find(
        (value) => value.id === fixture.servers[0].id,
      )?.puts,
    ).toBe(1);
    await page
      .getByRole("button", { name: "Отменить задание", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Отменено", exact: true })
      .waitFor();
    expect(
      (await fixture.stats()).find(
        (value) => value.id === fixture.servers[0].id,
      )?.puts,
    ).toBe(1);
  } finally {
    await fixture.close();
  }
}, 120000);
