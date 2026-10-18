import { expect, test } from "bun:test";
import { startFixture } from "./fixture";
import { signIn } from "./cleanup-steps";
import { decode } from "../../src/api";
import { instrumentEvents, sourceCounts } from "./instrumentation";

test("history is scoped, paginated and loaded only on its own page", async () => {
  const fixture = await startFixture({ historyRows: 51 });
  try {
    const page = await fixture.context.newPage();
    const requests: URL[] = [];
    page.on("request", (request) => requests.push(new URL(request.url())));
    await signIn(page, fixture.origin);
    expect(
      requests.filter((url) => url.pathname.endsWith("/history")),
    ).toHaveLength(0);
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await page
      .getByRole("heading", { name: "История очистки", exact: true })
      .waitFor({ timeout: 3000 });
    await page.getByRole("row").nth(50).waitFor();
    expect(await page.locator("tbody tr").count()).toBe(50);
    await page
      .getByRole("button", { name: "Следующая страница", exact: true })
      .click();
    await page.getByText("Страница 2", { exact: true }).waitFor();
    expect(await page.locator("tbody tr").count()).toBe(1);
    const scoped = requests.filter(
      (url) =>
        url.pathname === `/api/v1/servers/${fixture.servers[0].id}/history`,
    );
    // The initial SSE resync may reread the current page once.
    expect(scoped.length).toBeGreaterThanOrEqual(2);
    expect(scoped.length).toBeLessThanOrEqual(3);
    expect(scoped.every((url) => url.searchParams.get("limit") === "50")).toBe(
      true,
    );
    expect(scoped.some((url) => url.searchParams.has("cursor"))).toBe(true);
    await page
      .getByRole("button", { name: "Предыдущая страница", exact: true })
      .click();
    await page.getByText("Страница 1", { exact: true }).waitFor();
    expect(await page.locator("tbody tr").count()).toBe(50);
    await page
      .getByLabel("Сервер", { exact: true })
      .selectOption(fixture.servers[1].id);
    await page.getByText("Запусков пока нет", { exact: true }).waitFor();
    expect(new URL(page.url()).searchParams.get("server")).toBe(
      fixture.servers[1].id,
    );
    expect(
      requests.some(
        (url) =>
          url.pathname === `/api/v1/servers/${fixture.servers[1].id}/history`,
      ),
    ).toBe(true);
    expect(
      requests.filter(
        (url) =>
          url.pathname.includes("/profile") ||
          url.pathname.includes("/settings/session"),
      ),
    ).toHaveLength(0);
    await page.reload();
    await page.getByText("Запусков пока нет", { exact: true }).waitFor();
    expect(await page.getByLabel("Сервер", { exact: true }).inputValue()).toBe(
      fixture.servers[1].id,
    );
  } finally {
    await fixture.close();
  }
}, 120000);

test("a history failure is visible and never reported as an empty history", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.route("**/api/v1/servers/*/history?*", (route) =>
      route.fulfill({
        status: 503,
        contentType: "application/json",
        body: JSON.stringify({
          code: "unavailable",
          message: "Fixture unavailable",
          correlation_id: "11111111-1111-4111-8111-111111111111",
        }),
      }),
    );
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await page
      .getByRole("alert")
      .filter({ hasText: "11111111-1111-4111-8111-111111111111" })
      .waitFor({ timeout: 3000 });
    expect(
      await page.getByText("Запусков пока нет", { exact: true }).count(),
    ).toBe(0);
    expect(await page.locator("tbody tr").count()).toBe(0);
    await page.unroute("**/api/v1/servers/*/history?*");
    await page
      .getByRole("button", { name: "Повторить загрузку", exact: true })
      .click();
    await page.getByText("Запусков пока нет", { exact: true }).waitFor();
    expect(await page.getByRole("alert").count()).toBe(0);
  } finally {
    await fixture.close();
  }
}, 120000);

test("history is redacted after logout and Back navigation", async () => {
  const fixture = await startFixture({ historyRows: 51 });
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await page.getByRole("row").nth(50).waitFor();
    await page.getByRole("button", { name: "Выйти", exact: true }).click();
    await page.getByLabel("Логин", { exact: true }).waitFor();
    await page.goBack();
    await page
      .getByRole("link", { name: "Войти в центр", exact: true })
      .waitFor();
    expect(await page.locator("tbody tr").count()).toBe(0);
    expect(await page.getByLabel("Сервер", { exact: true }).count()).toBe(0);
  } finally {
    await fixture.close();
  }
}, 120000);

test("a delayed page from the previous server cannot replace the selected history", async () => {
  const fixture = await startFixture({ historyRows: 51 });
  const responseReady = Promise.withResolvers<void>();
  const releaseResponse = Promise.withResolvers<void>();
  const responseFinished = Promise.withResolvers<void>();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    const captured: unknown = await page.evaluate(async (serverId) => {
      const response = await fetch(
        `/api/v1/servers/${serverId}/history?limit=50`,
      );
      if (!response.ok) throw new Error("Fixture history request failed");
      return response.json();
    }, fixture.servers[0].id);
    await page.route(
      `**/api/v1/servers/${fixture.servers[0].id}/history?*`,
      async (route) => {
        responseReady.resolve();
        await releaseResponse.promise;
        try {
          await route.fulfill({ json: captured });
        } finally {
          responseFinished.resolve();
        }
      },
    );
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await responseReady.promise;
    await page
      .getByLabel("Сервер", { exact: true })
      .selectOption(fixture.servers[1].id);
    await page.getByText("Запусков пока нет", { exact: true }).waitFor();
    releaseResponse.resolve();
    await responseFinished.promise;
    expect(await page.getByLabel("Сервер", { exact: true }).inputValue()).toBe(
      fixture.servers[1].id,
    );
    expect(await page.locator("tbody tr").count()).toBe(0);
    expect(
      await page.getByText("Запусков пока нет", { exact: true }).isVisible(),
    ).toBe(true);
  } finally {
    releaseResponse.resolve();
    await fixture.close();
  }
}, 120000);

test("an invalid explicit history target never silently selects another server", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    const historyRequests: string[] = [];
    page.on("request", (request) => {
      if (new URL(request.url()).pathname.endsWith("/history"))
        historyRequests.push(request.url());
    });
    for (const server of [
      "malformed-id",
      "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    ]) {
      await page.goto(`${fixture.origin}/history/?server=${server}`);
      await page
        .getByRole("alert")
        .getByText(/идентификатор|сервер не найден/)
        .waitFor();
      expect(await page.locator("tbody tr").count()).toBe(0);
    }
    expect(historyRequests).toHaveLength(0);
  } finally {
    await fixture.close();
  }
}, 120000);

test("open history follows job completion and releases its sole event source on target and auth changes", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await instrumentEvents(page);
    await signIn(page, fixture.origin);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page.getByText("Выполняется", { exact: true }).waitFor();
    const historyRequests: string[] = [];
    const reads: Array<
      Readonly<{
        at: number;
        status: number;
        phases: ReadonlyArray<string | null>;
        slots: ReadonlyArray<boolean>;
      }>
    > = [];
    const captures: Promise<void>[] = [];
    page.on("response", (response) => {
      if (!new URL(response.url()).pathname.endsWith("/history")) return;
      const at = Date.now();
      const capture = response
        .json()
        .then((input: unknown) => {
          const data = decode("history", input);
          reads.push({
            at,
            status: response.status(),
            phases: data.jobs.map((job) => job.snapshot?.phase ?? null),
            slots: data.jobs.map((job) => job.slot_held),
          });
        })
        .catch(() => {
          reads.push({ at, status: response.status(), phases: [], slots: [] });
        });
      captures.push(capture);
    });
    page.on("request", (request) => {
      if (new URL(request.url()).pathname.endsWith("/history"))
        historyRequests.push(request.url());
    });
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await page
      .locator("tbody tr")
      .getByText("Выполняется", { exact: true })
      .waitFor();
    expect((await sourceCounts(page))?.active).toBe(1);
    for (let iteration = 0; iteration < 6; iteration++) {
      await fixture.phase(fixture.servers[0].id, "running");
    }
    const beforeFinish = { at: Date.now(), stats: await fixture.stats() };
    await fixture.finishLatest(fixture.servers[0].id);
    try {
      await page
        .locator("tbody tr")
        .getByText("Завершено", { exact: true })
        .waitFor({ timeout: 5000 });
    } catch (failure) {
      await Promise.all(captures);
      const current: unknown = await page.evaluate(async (serverId) => {
        const response = await fetch(
          `/api/v1/servers/${serverId}/history?limit=50`,
        );
        return response.json();
      }, fixture.servers[0].id);
      const data = decode("history", current);
      console.error(
        JSON.stringify({
          boundary: "history-live-timeout",
          beforeFinish,
          at: Date.now(),
          receipt: await fixture.receipt(fixture.servers[0].id),
          stats: await fixture.stats(),
          reads,
          events: await page.evaluate(() => window.__mailJobEvents),
          current: data.jobs.map((job) => ({
            phase: job.snapshot?.phase,
            slotHeld: job.slot_held,
            revision: job.revision,
          })),
          rendered: await page.locator("tbody").innerText(),
        }),
      );
      throw failure;
    }
    console.info(
      JSON.stringify({
        boundary: "history-live-completed",
        beforeFinish,
        at: Date.now(),
        reads,
        events: await page.evaluate(() => window.__mailJobEvents),
      }),
    );
    // Initial read/resync, first observed job, and terminal result stay bounded.
    expect(historyRequests.length).toBeLessThanOrEqual(4);
    expect((await sourceCounts(page))?.active).toBe(1);
    await page
      .getByLabel("Сервер", { exact: true })
      .selectOption(fixture.servers[1].id);
    await page.getByText("Запусков пока нет", { exact: true }).waitFor();
    expect(await sourceCounts(page)).toEqual({ active: 1, created: 2 });
    await page.evaluate(() =>
      window.dispatchEvent(new PageTransitionEvent("pagehide")),
    );
    expect((await sourceCounts(page))?.active).toBe(0);
    expect(await page.locator("tbody tr").count()).toBe(0);
    await page.evaluate(() =>
      window.dispatchEvent(new PageTransitionEvent("pageshow")),
    );
    await page.getByText("Запусков пока нет", { exact: true }).waitFor();
    expect(await sourceCounts(page)).toEqual({ active: 1, created: 3 });
  } finally {
    await fixture.close();
  }
}, 120000);

test("footer maps each server's actual agent observation and clears expanded details on auth reset", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    const agentRequests: string[] = [];
    page.on("request", (request) => {
      if (new URL(request.url()).pathname === "/api/v1/agents")
        agentRequests.push(request.url());
    });
    const agentsResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname === "/api/v1/agents" && response.ok(),
    );
    await signIn(page, fixture.origin);
    const agents = decode("agents", await (await agentsResponse).json());
    expect(agents).toHaveLength(2);
    const footer = page.locator("footer");
    await footer
      .locator("[data-agents-list] li")
      .nth(1)
      .waitFor({ state: "attached" });
    const requestsBeforeExpansion = agentRequests.length;
    await footer.locator("summary").click();
    for (const agent of agents) {
      const row = footer.locator(
        `[data-agents-list] [data-server-id="${agent.server_id}"]`,
      );
      expect(await row.getByText(agent.name, { exact: true }).isVisible()).toBe(
        true,
      );
      expect(await row.textContent()).toContain(
        agent.health.version ?? "Версия неизвестна",
      );
      if (agent.health.observed_at)
        expect(await row.locator("time").getAttribute("datetime")).toBe(
          agent.health.observed_at,
        );
      else expect(await row.textContent()).toContain("Контакт ещё не получен");
    }
    expect(agentRequests.length).toBe(requestsBeforeExpansion);
    const hostileName =
      '<img data-footer-test src=x onerror="document.body.dataset.injected=true">';
    await page.route("**/api/v1/agents", async (route) => {
      await route.fulfill({
        json: agents.map((agent, index) =>
          index === 0 ? { ...agent, name: hostileName } : agent,
        ),
      });
    });
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await footer
      .locator("[data-agents-list] li")
      .nth(1)
      .waitFor({ state: "attached" });
    await footer.locator("summary").click();
    expect(
      await footer.getByText(hostileName, { exact: true }).isVisible(),
    ).toBe(true);
    expect(await footer.locator("img").count()).toBe(0);
    await page.evaluate(() =>
      window.dispatchEvent(new PageTransitionEvent("pagehide")),
    );
    expect(await footer.locator("[data-agents-list] li").count()).toBe(0);
    expect(await footer.locator("details").getAttribute("open")).toBeNull();
    for (const agent of agents)
      expect(await footer.textContent()).not.toContain(agent.name);
    expect(await footer.textContent()).not.toContain(hostileName);
  } finally {
    await fixture.close();
  }
}, 120000);

test("history refreshes a terminal phase while service verification still holds the slot", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await instrumentEvents(page);
    await signIn(page, fixture.origin);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page.getByText("Выполняется", { exact: true }).waitFor();
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await page
      .locator("tbody tr")
      .getByText("Выполняется", { exact: true })
      .waitFor();
    const watchDeadline = Date.now() + 3000;
    while (
      !(await fixture.stats()).some(
        (agent) =>
          agent.id === fixture.servers[0].id && agent.subscriptions > 0,
      )
    ) {
      if (Date.now() >= watchDeadline)
        throw new Error("Submitted job has no agent watch");
      await Bun.sleep(20);
    }
    const progressRead = page.waitForResponse(async (response) => {
      if (
        !new URL(response.url()).pathname.endsWith("/history") ||
        !response.ok()
      )
        return false;
      return decode("history", await response.json()).jobs.some(
        (job) => job.snapshot?.revision === "2",
      );
    });
    await fixture.phase(fixture.servers[0].id, "running");
    await page.waitForFunction(() =>
      window.__mailJobEvents?.some((event) => event.phase === "running"),
    );
    await progressRead;
    await fixture.mailState(fixture.servers[0].id, "unknown");
    await fixture.finishLatest(fixture.servers[0].id);
    await page.waitForFunction(() =>
      window.__mailJobEvents?.some(
        (event) => event.phase === "completed" && event.slotHeld === true,
      ),
    );
    const current: unknown = await page.evaluate(async (serverId) => {
      const response = await fetch(
        `/api/v1/servers/${serverId}/history?limit=50`,
      );
      return response.json();
    }, fixture.servers[0].id);
    const heldTerminal = decode("history", current).jobs[0];
    expect(heldTerminal?.snapshot?.phase).toBe("completed");
    expect(heldTerminal?.slot_held).toBe(true);
    await page
      .locator("tbody tr")
      .getByText("Состояние службы не подтверждено", { exact: true })
      .waitFor({ timeout: 3000 });
    await page
      .locator("tbody tr")
      .getByText("Завершено", { exact: true })
      .waitFor({ timeout: 3000 });
  } finally {
    await fixture.close();
  }
}, 120000);
