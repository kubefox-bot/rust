import { expect, test } from "bun:test";
import { decode, type BrowserContracts } from "../../src/api";
import { startFixture } from "./fixture";
import { signIn } from "./cleanup-steps";

test("SSH host-key confirmation precedes credential entry", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/`);
    await page
      .getByRole("button", { name: "Добавить сервер", exact: true })
      .click();
    expect(await page.getByLabel("Пароль SSH", { exact: true }).count()).toBe(
      0,
    );
    const requests: unknown[] = [];
    await page.route("**/api/v1/enrollment/probe", async (route) => {
      requests.push(route.request().postDataJSON());
      await route.fulfill({
        json: {
          target: { host: "127.0.0.1", port: 22 },
          fingerprint: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        },
      });
    });
    await page.getByLabel("Хост SSH", { exact: true }).fill("127.0.0.1");
    await page
      .getByRole("button", { name: "Проверить SSH-ключ", exact: true })
      .click();
    await page
      .getByText("SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", {
        exact: true,
      })
      .waitFor();
    expect(requests).toEqual([{ host: "127.0.0.1", port: 22 }]);
    expect(await page.getByLabel("Пароль SSH", { exact: true }).count()).toBe(
      0,
    );
    expect(
      await page
        .getByRole("button", { name: "Продолжить", exact: true })
        .isDisabled(),
    ).toBe(true);
    await page.getByLabel("Я сверил SSH-ключ сервера", { exact: true }).check();
    await page.getByRole("button", { name: "Продолжить", exact: true }).click();
    await page.getByLabel("Пароль SSH", { exact: true }).waitFor();
  } finally {
    await fixture.close();
  }
}, 120000);

test("lost installation response clears credentials and reconciles the same operation", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/`);
    const packages = decode(
      "packages",
      await page.evaluate(async () => {
        const response = await fetch("/api/v1/enrollment/packages");
        if (!response.ok)
          throw new Error("Fixture package catalog is unavailable");
        return response.json();
      }),
    );
    const packageView = packages[0];
    if (!packageView) throw new Error("Trusted fixture package is required");
    await page
      .getByRole("button", { name: "Добавить сервер", exact: true })
      .click();
    await page.route("**/api/v1/enrollment/probe", async (route) => {
      await route.fulfill({
        json: {
          target: { host: "127.0.0.1", port: 22 },
          fingerprint: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        },
      });
    });
    const installs: BrowserContracts["install"][] = [];
    const reconciliations: string[] = [];
    await page.route("**/api/v1/enrollment/install", async (route) => {
      installs.push(decode("install", route.request().postDataJSON()));
      await route.abort("connectionreset");
    });
    await page.route("**/api/v1/enrollment/*/reconcile", async (route) => {
      const installation = installs[0];
      if (!installation)
        throw new Error("No installation request to reconcile");
      reconciliations.push(new URL(route.request().url()).pathname);
      await route.fulfill({
        json: decode("enrollment", {
          id: installation.operation_id,
          agent_id: installation.agent_id,
          server_id: installation.server_id,
          profile_id: installation.profile_id,
          fingerprint: installation.fingerprint,
          package_version: packageView.build,
          expected_sha256: packageView.sha256,
          phase: "ready",
          slot_held: false,
        }),
      });
    });
    await page
      .getByLabel("Название сервера", { exact: true })
      .fill("Synthetic enrollment target");
    await page.getByLabel("Хост SSH", { exact: true }).fill("127.0.0.1");
    await page
      .getByRole("button", { name: "Проверить SSH-ключ", exact: true })
      .click();
    await page.getByLabel("Я сверил SSH-ключ сервера", { exact: true }).check();
    await page.getByRole("button", { name: "Продолжить", exact: true }).click();
    await page
      .getByLabel("Пользователь SSH", { exact: true })
      .fill("Administrator");
    await page
      .getByLabel("Пароль SSH", { exact: true })
      .fill("ssh-credential-sentinel");
    await page
      .getByRole("button", { name: "Установить агента", exact: true })
      .click();
    await page
      .getByText("Результат установки неизвестен", { exact: true })
      .waitFor();
    expect(await page.getByLabel("Пароль SSH", { exact: true }).count()).toBe(
      0,
    );
    expect(
      await page
        .getByRole("button", { name: "Установить агента", exact: true })
        .count(),
    ).toBe(0);
    expect(installs).toHaveLength(1);
    const installation = installs[0];
    if (!installation) throw new Error("Installation request is required");
    await page
      .getByRole("button", { name: "Согласовать состояние", exact: true })
      .click();
    await page.getByText("Требуется настройка", { exact: true }).waitFor();
    expect(reconciliations).toEqual([
      `/api/v1/enrollment/${installation.operation_id}/reconcile`,
    ]);
    expect(installs).toHaveLength(1);
    expect(
      await page.evaluate(() =>
        JSON.stringify({
          local: { ...localStorage },
          session: { ...sessionStorage },
        }),
      ),
    ).not.toContain("ssh-credential-sentinel");
  } finally {
    await fixture.close();
  }
}, 120000);

test("a host-key challenge for another target cannot unlock credentials", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/`);
    await page
      .getByRole("button", { name: "Добавить сервер", exact: true })
      .click();
    await page.route("**/api/v1/enrollment/probe", async (route) => {
      await route.fulfill({
        json: {
          target: { host: "other.example", port: 22 },
          fingerprint: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        },
      });
    });
    await page.getByLabel("Хост SSH", { exact: true }).fill("127.0.0.1");
    await page
      .getByRole("button", { name: "Проверить SSH-ключ", exact: true })
      .click();
    await page
      .getByRole("alert")
      .filter({ hasText: "Ответ относится к другому SSH-серверу" })
      .waitFor();
    expect(
      await page
        .getByLabel("Я сверил SSH-ключ сервера", { exact: true })
        .count(),
    ).toBe(0);
    expect(await page.getByLabel("Пароль SSH", { exact: true }).count()).toBe(
      0,
    );
  } finally {
    await fixture.close();
  }
}, 120000);

test("a rejected busy installation returns to confirmed credentials without resubmitting", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page.getByText("Выполняется", { exact: true }).waitFor();
    await page.goto(`${fixture.origin}/settings/`);
    await page
      .getByRole("button", { name: "Добавить сервер", exact: true })
      .click();
    await page.route("**/api/v1/enrollment/probe", async (route) => {
      await route.fulfill({
        json: {
          target: { host: "127.0.0.1", port: 22 },
          fingerprint: "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        },
      });
    });
    let installRequests = 0;
    page.on("request", (request) => {
      if (new URL(request.url()).pathname === "/api/v1/enrollment/install")
        installRequests += 1;
    });
    await page.getByLabel("Хост SSH", { exact: true }).fill("127.0.0.1");
    await page
      .getByRole("button", { name: "Проверить SSH-ключ", exact: true })
      .click();
    await page.getByLabel("Я сверил SSH-ключ сервера", { exact: true }).check();
    await page.getByRole("button", { name: "Продолжить", exact: true }).click();
    await page
      .getByLabel("Пользователь SSH", { exact: true })
      .fill("Administrator");
    await page
      .getByLabel("Пароль SSH", { exact: true })
      .fill("rejected-credential-sentinel");
    await page
      .getByRole("button", { name: "Установить агента", exact: true })
      .click();
    await page
      .getByRole("alert")
      .filter({ hasText: "другая операция уже выполняется" })
      .waitFor({ timeout: 3000 });
    expect(
      await page.getByLabel("Пароль SSH", { exact: true }).inputValue(),
    ).toBe("");
    expect(
      await page.getByLabel("Пользователь SSH", { exact: true }).inputValue(),
    ).toBe("");
    expect(
      await page
        .getByText("Результат установки неизвестен", { exact: true })
        .count(),
    ).toBe(0);
    expect(installRequests).toBe(1);
    await page
      .getByRole("button", { name: "Установить агента", exact: true })
      .click();
    expect(installRequests).toBe(1);
    expect(
      await page.getByLabel("Пароль SSH", { exact: true }).inputValue(),
    ).toBe("");
  } finally {
    await fixture.close();
  }
}, 120000);

test("revoking an agent cannot offer SSH recovery as a way to restore revoked trust", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(`${fixture.origin}/settings/`);
    await page
      .getByRole("button", { name: "Добавить сервер", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Отозвать доступ", exact: true })
      .click();
    const dialog = page.getByRole("dialog");
    await dialog.getByText(fixture.servers[0].name, { exact: true }).waitFor();
    await dialog
      .getByLabel("Подтверждаю действие для этого сервера", { exact: true })
      .check();
    await dialog
      .getByRole("button", { name: "Подтвердить", exact: true })
      .click();
    await page.getByText("Доступ отозван", { exact: true }).waitFor();
    expect(
      await page
        .getByRole("button", { name: "Восстановить доверие", exact: true })
        .isDisabled(),
    ).toBe(true);
    expect(
      await page
        .getByRole("button", { name: "Обновить агента", exact: true })
        .isDisabled(),
    ).toBe(true);
    expect(
      await page
        .getByRole("button", { name: "Обновить сертификат", exact: true })
        .isDisabled(),
    ).toBe(true);
  } finally {
    await fixture.close();
  }
}, 120000);
