import { expect, test } from "bun:test";
import { startFixture } from "./fixture";
import { signIn } from "./cleanup-steps";

test("running preview opens a bounded searchable file dialog without enabling cleanup", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    const queries: URL[] = [];
    let completed = false;
    await page.route("**/api/v1/jobs/*/files?*", async (route) => {
      const url = new URL(route.request().url());
      queries.push(url);
      const offset = Number(url.searchParams.get("offset"));
      const search = url.searchParams.get("search") ?? "";
      const jobId = url.pathname.split("/").at(-2);
      const total = search ? 1 : 101;
      await route.fulfill({
        json: {
          job_id: jobId,
          total: String(total),
          revision: "1",
          complete: completed,
          rows: Array.from(
            { length: Math.min(50, total - offset) },
            (_, index) => ({
              id: `file-${offset + index}`,
              path: `C:\\Mail\\${search || offset + index}.eml`,
              size_bytes: "9007199254740993",
              created_at: "2026-01-02T03:04:05Z",
              orphan: index % 2 === 0,
            }),
          ),
        },
      });
    });
    await signIn(page, fixture.origin);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Выполняется", exact: true })
      .waitFor();
    expect(await page.getByRole("dialog").count()).toBe(0);
    expect(
      await page
        .getByRole("button", { name: "Очистить", exact: true })
        .isEnabled(),
    ).toBe(false);
    await page
      .getByRole("button", { name: "Подробности", exact: true })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Выбранные файлы",
      exact: true,
    });
    await dialog.getByText("C:\\Mail\\0.eml", { exact: true }).waitFor();
    // Wait beyond the former 250 ms mount refresh to detect a duplicate request.
    await page.waitForTimeout(350);
    expect(queries).toHaveLength(1);
    expect(await dialog.locator("tbody tr").count()).toBe(50);
    expect(await dialog.locator("thead tr").count()).toBe(1);
    expect(
      (await dialog.getByRole("columnheader").allTextContents()).map((text) =>
        text.replace(/\s+/g, " ").trim(),
      ),
    ).toEqual(["Файл / путь ↑", "Размер ↕", "Создан ↕", "Связь с базой"]);
    expect(
      await dialog.getByRole("columnheader").nth(0).getAttribute("aria-sort"),
    ).toBe("ascending");
    expect(
      await dialog
        .locator("tbody tr")
        .first()
        .locator("td")
        .nth(1)
        .getAttribute("title"),
    ).toBe("9007199254740993 Б");
    await dialog.getByRole("button", { name: "Далее", exact: true }).click();
    await dialog.getByText("C:\\Mail\\50.eml", { exact: true }).waitFor();
    await dialog.getByLabel("Поиск по пути", { exact: true }).fill("needle%_");
    await dialog.getByText("C:\\Mail\\needle%_.eml", { exact: true }).waitFor();
    expect(queries.at(-1)?.searchParams.get("offset")).toBe("0");
    expect(queries.at(-1)?.searchParams.get("search")).toBe("needle%_");
    const sorted = page.waitForResponse(
      (response) => new URL(response.url()).searchParams.get("sort") === "size",
    );
    await dialog.getByRole("button", { name: "Размер", exact: false }).click();
    await sorted;
    expect(
      await dialog.getByRole("columnheader").nth(1).getAttribute("aria-sort"),
    ).toBe("ascending");
    const descending = page.waitForResponse((response) => {
      const url = new URL(response.url());
      return (
        url.pathname.endsWith("/files") &&
        url.searchParams.get("sort") === "size" &&
        url.searchParams.get("direction") === "desc"
      );
    });
    await dialog.getByRole("button", { name: "Размер", exact: false }).click();
    await descending;
    expect(
      await dialog.getByRole("columnheader").nth(1).getAttribute("aria-sort"),
    ).toBe("descending");
    completed = true;
    await fixture.finishLatest(fixture.servers[0].id);
    await dialog.getByText("Сбор завершён", { exact: false }).waitFor();
    await dialog.press("Escape");
    expect(await page.getByRole("dialog").count()).toBe(0);
    await page
      .getByRole("button", { name: "Подробности", exact: true })
      .focus();
    await page
      .getByRole("button", { name: "Подробности", exact: true })
      .press("Enter");
    await dialog.waitFor();
    await dialog.getByRole("button", { name: "Закрыть", exact: true }).click();
  } finally {
    await fixture.close();
  }
}, 60000);

test("file metadata failure stays explicit and can be retried", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    let supported = false;
    await page.route("**/api/v1/jobs/*/files?*", async (route) => {
      if (!supported) {
        await route.fulfill({ status: 501, json: {} });
        return;
      }
      await route.fulfill({
        json: {
          job_id: new URL(route.request().url()).pathname.split("/").at(-2),
          rows: [],
          total: "0",
          revision: "1",
          complete: true,
        },
      });
    });
    await signIn(page, fixture.origin);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Подробности", exact: true })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Выбранные файлы",
      exact: true,
    });
    await dialog.getByRole("alert").waitFor();
    expect(
      await dialog.getByText("Подходящих файлов нет.", { exact: true }).count(),
    ).toBe(0);
    supported = true;
    await dialog
      .getByRole("button", { name: "Повторить", exact: true })
      .click();
    await dialog.getByText("Подходящих файлов нет.", { exact: true }).waitFor();
  } finally {
    await fixture.close();
  }
}, 60000);
