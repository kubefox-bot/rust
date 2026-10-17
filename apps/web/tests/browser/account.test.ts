import { expect, test } from "bun:test";
import { startFixture } from "./fixture";
import { signIn } from "./cleanup-steps";

test("session settings expose and persist exactly the agreed idle intervals", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/#account`);
    const timeout = page.getByLabel("Выход при бездействии", { exact: true });
    await timeout.waitFor({ timeout: 3000 });
    expect(
      await timeout
        .locator("option")
        .evaluateAll((options) =>
          options.map((option) => option.getAttribute("value")),
        ),
    ).toEqual(["15", "30", "60", "1440"]);
    await timeout.selectOption("60");
    await page
      .getByRole("button", { name: "Сохранить таймаут", exact: true })
      .click();
    await page.getByText("Таймаут сохранён", { exact: true }).waitFor();
    await page.reload();
    await page.waitForFunction(() => {
      const select = document.querySelector<HTMLSelectElement>("#account-idle");
      return select?.value === "60" && !select.disabled;
    });
    expect(
      await page
        .getByRole("button", { name: "Заблокировать", exact: true })
        .count(),
    ).toBe(0);
  } finally {
    await fixture.close();
  }
}, 120000);

test("password confirmation is required before committing and the new password requires ordinary login", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/#account`);
    await page
      .getByLabel("Текущий пароль", { exact: true })
      .waitFor({ timeout: 3000 });
    await page
      .getByLabel("Текущий пароль", { exact: true })
      .fill("fixture-center-password");
    await page
      .getByLabel("Новый пароль", { exact: true })
      .fill("replacement-center-password");
    await page
      .getByLabel("Повторите новый пароль", { exact: true })
      .fill("different-center-password");
    await page
      .getByRole("button", { name: "Изменить пароль", exact: true })
      .click();
    await page.getByText("Пароли не совпадают.", { exact: true }).waitFor();
    expect(
      await page.evaluate(
        async () =>
          (await (await fetch("/api/v1/auth/session")).json()).authenticated,
      ),
    ).toBe(true);
    await page
      .getByLabel("Текущий пароль", { exact: true })
      .fill("fixture-center-password");
    await page
      .getByLabel("Новый пароль", { exact: true })
      .fill("replacement-center-password");
    await page
      .getByLabel("Повторите новый пароль", { exact: true })
      .fill("replacement-center-password");
    await page
      .getByRole("button", { name: "Изменить пароль", exact: true })
      .click();
    await page.waitForURL(`${fixture.origin}/login/`);
    await page.getByLabel("Логин", { exact: true }).fill("admin");
    await page
      .getByLabel("Пароль", { exact: true })
      .fill("fixture-center-password");
    await page
      .getByRole("button", { name: "Войти в центр", exact: true })
      .click();
    await page.getByRole("alert").waitFor();
    expect(new URL(page.url()).pathname).toBe("/login/");
    await page
      .getByLabel("Пароль", { exact: true })
      .fill("replacement-center-password");
    await page
      .getByRole("button", { name: "Войти в центр", exact: true })
      .click();
    await page.getByLabel("Сервер", { exact: true }).waitFor();
  } finally {
    await fixture.close();
  }
}, 120000);

test("recovery candidate is shown once, never persisted, and requires retyping before commit", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/#account`);
    await page
      .getByLabel("Текущий пароль для замены ключа", { exact: true })
      .waitFor({ timeout: 3000 });
    await page
      .getByLabel("Текущий пароль для замены ключа", { exact: true })
      .fill("fixture-center-password");
    await page
      .getByRole("button", { name: "Создать новый ключ", exact: true })
      .click();
    const issued = page.getByLabel("Новый ключ восстановления", {
      exact: true,
    });
    await issued.waitFor();
    const recoveryKey = await issued.innerText();
    expect(recoveryKey).toMatch(/^hmc-recovery-v1:[A-Za-z0-9_-]{43}$/);
    expect(
      (await page.locator("body").innerText()).split(recoveryKey),
    ).toHaveLength(2);
    expect(
      await page
        .getByRole("button", { name: "Подтвердить новый ключ", exact: true })
        .count(),
    ).toBe(0);
    const stored = await page.evaluate(() =>
      JSON.stringify({
        local: { ...localStorage },
        session: { ...sessionStorage },
      }),
    );
    expect(stored).not.toContain(recoveryKey);
    expect(
      await page
        .getByLabel("Текущий пароль для замены ключа", { exact: true })
        .count(),
    ).toBe(0);
    await page
      .getByRole("button", { name: "Я сохранил ключ", exact: true })
      .click();
    expect(await issued.count()).toBe(0);
    expect(await page.locator("body").innerText()).not.toContain(recoveryKey);
    await page
      .getByLabel("Повторите ключ восстановления", { exact: true })
      .fill("wrong-recovery-secret");
    await page
      .getByRole("button", { name: "Подтвердить новый ключ", exact: true })
      .click();
    await page.getByRole("alert").waitFor();
    expect(await page.getByRole("alert").innerText()).not.toContain(
      "wrong-recovery-secret",
    );
    await page
      .getByLabel("Повторите ключ восстановления", { exact: true })
      .fill(recoveryKey);
    await page
      .getByRole("button", { name: "Подтвердить новый ключ", exact: true })
      .click();
    await page.waitForURL(`${fixture.origin}/login/`);
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/#account`);
    await page
      .getByRole("button", { name: "Создать новый ключ", exact: true })
      .waitFor();
    expect(
      await page
        .getByLabel("Новый ключ восстановления", { exact: true })
        .count(),
    ).toBe(0);
    expect(await page.locator("body").innerText()).not.toContain(recoveryKey);
    expect(
      await page.evaluate(() =>
        JSON.stringify({
          local: { ...localStorage },
          session: { ...sessionStorage },
        }),
      ),
    ).not.toContain(recoveryKey);
  } finally {
    await fixture.close();
  }
}, 120000);

test("leaving account settings clears uncommitted secrets and recovery candidate", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/#account`);
    await page
      .getByLabel("Текущий пароль для замены ключа", { exact: true })
      .waitFor({ timeout: 3000 });
    await page
      .getByLabel("Текущий пароль для замены ключа", { exact: true })
      .fill("fixture-center-password");
    await page
      .getByRole("button", { name: "Создать новый ключ", exact: true })
      .click();
    const issued = page.getByLabel("Новый ключ восстановления", {
      exact: true,
    });
    await issued.waitFor();
    const recoveryKey = await issued.innerText();
    await page
      .getByLabel("Новый пароль", { exact: true })
      .fill("uncommitted-password-secret");
    await page.evaluate(() => {
      window.location.hash = "agents";
    });
    await page
      .getByLabel("Выход при бездействии", { exact: true })
      .waitFor({ state: "detached" });
    await page.evaluate(() => {
      window.location.hash = "account";
    });
    await page.getByLabel("Выход при бездействии", { exact: true }).waitFor();
    expect(
      await page.getByLabel("Новый пароль", { exact: true }).inputValue(),
    ).toBe("");
    expect(await issued.count()).toBe(0);
    expect(await page.locator("body").innerText()).not.toContain(recoveryKey);
    expect(
      await page.evaluate(() =>
        JSON.stringify({
          local: { ...localStorage },
          session: { ...sessionStorage },
        }),
      ),
    ).not.toContain("uncommitted-password-secret");
  } finally {
    await fixture.close();
  }
}, 120000);

test("a login in another browser context revokes and clears the earlier account screen", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/#account`);
    await page
      .getByLabel("Текущий пароль для замены ключа", { exact: true })
      .waitFor({ timeout: 3000 });
    await page
      .getByLabel("Текущий пароль для замены ключа", { exact: true })
      .fill("fixture-center-password");
    await page
      .getByRole("button", { name: "Создать новый ключ", exact: true })
      .click();
    await page
      .getByLabel("Новый ключ восстановления", { exact: true })
      .waitFor();
    const browser = fixture.context.browser();
    if (!browser) throw new Error("The browser fixture has no browser owner.");
    const anotherContext = await browser.newContext();
    try {
      const anotherPage = await anotherContext.newPage();
      await signIn(anotherPage, fixture.origin);
      await page
        .getByLabel("Новый ключ восстановления", { exact: true })
        .waitFor({ state: "detached", timeout: 45000 });
      expect(
        await page.getByLabel("Текущий пароль", { exact: true }).count(),
      ).toBe(0);
      expect(
        await page.evaluate(
          async () =>
            (await (await fetch("/api/v1/auth/session")).json()).authenticated,
        ),
      ).toBe(false);
    } finally {
      await anotherContext.close();
    }
  } finally {
    await fixture.close();
  }
}, 120000);
