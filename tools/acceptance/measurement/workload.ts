import { decode } from "../../../apps/web/src/api";
import { statsMessage } from "../../../apps/web/tests/browser/fixture-protocol";
import { selectValue } from "../../../apps/web/tests/browser/controls";
import {
  FIXTURE_PASSWORD,
  PREVIEW_STEPS,
  UI_TIMEOUT_MS,
  AGENT_CONTACT_RETRY_MS,
  HISTORY_PAGE_ROWS,
} from "./conts";
import { MAX_JOURNAL_ROWS } from "../conts";
import { fullProtocolPreview } from "./full-preview";
import { browserSnapshot, verifyLazyPages } from "./browser";
import { startResourceFixture } from "./fixture";
import { deadline } from "./deadline";
import { sampleWindow } from "./process";
import type {
  BrowserSnapshot,
  MeasurementConfig,
  ProcessWindow,
} from "./types";

const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const assertAgentContacts = (value: unknown): void => {
  if (
    !record(value) ||
    !Array.isArray(value.stats) ||
    value.stats.length !== 2 ||
    !value.stats.every(
      (entry: unknown) =>
        record(entry) &&
        typeof entry.health_calls === "number" &&
        entry.health_calls > 0,
    )
  )
    throw new Error(
      "Both TLS fixtures must receive actual release-center health requests",
    );
};
export const measureWorkload = async (config: MeasurementConfig) => {
  const fixture = await startResourceFixture(config);
  const windows: ProcessWindow[] = [];
  const browser: BrowserSnapshot[] = [];
  const measure = async (name: string, operation: () => Promise<void>) => {
    const result = await sampleWindow({
      name,
      pid: fixture.centerPid,
      root: config.root,
      options: config,
      operation,
    });
    windows.push(result);
    console.log(
      `${name}: sampled peak RSS ${result.summary.rssMaxBytes} bytes; CPU ${result.summary.cpuAveragePercentOfOneCore.toFixed(2)}% of one core`,
    );
  };
  try {
    await measure("center_idle_locked", async () => {});
    const page = await fixture.context.newPage();
    page.setDefaultTimeout(UI_TIMEOUT_MS);
    const initialRequests: string[] = [];
    const recordRequest = (request: Readonly<{ url: () => string }>) =>
      initialRequests.push(new URL(request.url()).pathname);
    page.on("request", recordRequest);
    await page.goto(`${fixture.origin}/login/`);
    await page.getByLabel("Логин", { exact: true }).fill("admin");
    await page.getByLabel("Пароль", { exact: true }).fill(FIXTURE_PASSWORD);
    browser.push(await browserSnapshot({ name: "initial_login", page }));
    const webBuildVersion = (await page.locator("footer .build").textContent())
      ?.replace(/^\s*·\s*/, "")
      .trim();
    if (webBuildVersion !== fixture.centerBuildVersion)
      throw new Error(
        `Center/web build mismatch: ${fixture.centerBuildVersion} / ${webBuildVersion}`,
      );
    await measure("center_login_kdf_and_initial_ui", async () => {
      await page
        .getByRole("button", { name: "Войти в центр", exact: true })
        .click();
      await selectValue(page, {
        label: "Сервер",
        value: fixture.servers[0].id,
      });
      await page
        .getByRole("button", { name: "Предпросмотр", exact: true })
        .click({ trial: true });
    });
    const cleanup = await browserSnapshot({ name: "initial_cleanup", page });
    browser.push(cleanup);
    page.off("request", recordRequest);
    await deadline(
      (async () => {
        while (true) {
          const values: unknown = await page.evaluate(async () => {
            const response = await fetch("/api/v1/agents");
            if (!response.ok) throw new Error("Agent observations unavailable");
            return response.json();
          });
          const agents = decode("agents", values);
          if (
            agents.length === 2 &&
            agents.every((agent) => agent.health.version === "tls-fixture")
          )
            break;
          await Bun.sleep(AGENT_CONTACT_RETRY_MS);
        }
      })(),
      UI_TIMEOUT_MS,
    );
    assertAgentContacts(await fixture.stats());
    // Two independently selected UI targets keep one real center connection per page.
    const other = await fixture.context.newPage();
    await other.goto(`${fixture.origin}/?server=${fixture.servers[1].id}`);
    await other
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click({ trial: true });
    await measure("center_two_tls_agents_two_open_ui_tabs", async () => {});
    browser.push(
      await browserSnapshot({ name: "second_cleanup_tab", page: other }),
    );
    await other.close();
    for (const scan of PREVIEW_STEPS) {
      if (scan === "full") await fixture.stopMail(fixture.servers[0].id);
      await selectValue(page, { label: "Режим сканирования", value: scan });
      await measure(`center_${scan}_preview_protocol_fixture`, async () => {
        // The shared TLS fixture profile validator always reports Running. Full mode
        // therefore measures real center protocol dispatch, followed by the real UI observer.
        if (scan === "full")
          await fullProtocolPreview({ page, serverId: fixture.servers[0].id });
        else
          await page
            .getByRole("button", { name: "Предпросмотр", exact: true })
            .click();
        await page
          .getByRole("heading", { name: "Выполняется", exact: true })
          .waitFor();
        await fixture.finish(fixture.servers[0].id);
        await page
          .getByRole("button", { name: "Очистить", exact: true })
          .click({ trial: true });
      });
      browser.push(
        await browserSnapshot({ name: `${scan}_preview_completed`, page }),
      );
    }
    await selectValue(page, { label: "Режим сканирования", value: "fast" });
    await measure("center_cancel_protocol_fixture", async () => {
      await page
        .getByRole("button", { name: "Предпросмотр", exact: true })
        .click();
      await page
        .getByRole("heading", { name: "Выполняется", exact: true })
        .waitFor();
      await page
        .getByRole("button", { name: "Отменить задание", exact: true })
        .click();
      await page
        .getByRole("heading", { name: "Отменено", exact: true })
        .waitFor();
    });
    browser.push(await browserSnapshot({ name: "cancelled", page }));
    await page.goto(
      `${fixture.origin}/history/?server=${fixture.servers[0].id}`,
    );
    await page
      .locator("tbody tr")
      .nth(HISTORY_PAGE_ROWS - 1)
      .waitFor();
    const history = await browserSnapshot({ name: "history_first_page", page });
    browser.push(history);
    await page.goto(
      `${fixture.origin}/settings/?server=${fixture.servers[0].id}`,
    );
    await page.getByLabel("Хост MySQL", { exact: true }).waitFor();
    const settings = await browserSnapshot({ name: "settings_selected", page });
    browser.push(settings);
    const lazy = verifyLazyPages({
      cleanup,
      history,
      settings,
      initialRequests,
    });
    const counters = await fixture.stats();
    const stats = statsMessage(counters);
    if (
      stats[0]?.puts !== 3 ||
      stats[0]?.cancels !== 1 ||
      stats[1]?.puts !== 0 ||
      browser.some(
        (item) =>
          item.journalDomRows > MAX_JOURNAL_ROWS ||
          item.historyDomRows > HISTORY_PAGE_ROWS,
      )
    )
      throw new Error(
        "Measured workload counters or bounded DOM assertions failed",
      );
    return {
      windows,
      browser,
      lazy,
      initialRequests,
      counters,
      centerBuildVersion: fixture.centerBuildVersion,
      webBuildVersion,
      browserVersion: fixture.browserVersion,
      centerPid: fixture.centerPid,
    };
  } finally {
    await fixture.close();
  }
};
