import { selectValue, selectDateTime } from "./controls";
import { expect, test } from "bun:test";
import { startFixture } from "./fixture";

test("reloaded fractional preview preserves the exact cutoff until the operator edits it", async () => {
  const { signIn } = await import("./cleanup-steps");
  const { decode } = await import("../../src/api");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    const serverId = fixture.servers[0].id;
    const context = decode(
      "cleanup_context",
      await page.evaluate(async (id) => {
        const response = await fetch(`/api/v1/servers/${id}/cleanup-context`);
        if (!response.ok) throw new Error("Cleanup context unavailable");
        return response.json();
      }, serverId),
    );
    const exactLocal = "2026-09-18T02:55:44.049364123";
    const cutoff = decode(
      "cutoff",
      await page.evaluate(
        async ({ id, local }) => {
          const response = await fetch(`/api/v1/servers/${id}/resolve`, {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-HMail-Request": "1",
            },
            body: JSON.stringify({ kind: "before", local }),
          });
          if (!response.ok) throw new Error("Exact cutoff was rejected");
          return response.json();
        },
        { id: serverId, local: exactLocal },
      ),
    );
    expect(cutoff.local).toBe(exactLocal);
    const intent = decode("submit", {
      job_id: crypto.randomUUID(),
      server_id: serverId,
      profile_version: context.profile_version,
      operation: "preview",
      scan: "full",
      resolved_cutoff: cutoff,
      service_policy: "require_stopped",
      limits: { batch_size: 1000, workers: 2 },
      preview_job_id: null,
      confirmed: false,
    });
    const original = decode(
      "job",
      await page.evaluate(async (body) => {
        const response = await fetch(`/api/v1/servers/${body.server_id}/jobs`, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-HMail-Request": "1",
          },
          body: JSON.stringify(body),
        });
        if (!response.ok) throw new Error("Exact preview was rejected");
        return response.json();
      }, intent),
    );
    await fixture.finishLatest(serverId);
    await page.reload();
    await page
      .getByRole("heading", { name: "Завершено", exact: true })
      .waitFor();
    const field = page.getByRole("button", {
      name: "Граница по времени сервера",
      exact: true,
    });
    expect(await field.getAttribute("data-datetime-value")).toBe(exactLocal);
    await field.click();
    const picker = page.getByRole("dialog", {
      name: "Граница по времени сервера",
      exact: true,
    });
    await picker
      .getByRole("spinbutton", { name: "Год календаря" })
      .press("Enter");
    expect(await picker.isVisible()).toBe(true);
    expect(await field.getAttribute("data-datetime-value")).toBe(exactLocal);
    await picker.getByRole("button", { name: "Отмена", exact: true }).click();
    expect(
      await field.evaluate(
        (input) =>
          input instanceof HTMLButtonElement && input.form?.checkValidity(),
      ),
    ).toBe(true);
    const cutoffDisplay = page
      .getByText("Граница:", { exact: false })
      .locator("time");
    await cutoffDisplay.waitFor();
    expect(await cutoffDisplay.getAttribute("datetime")).toBe(exactLocal);
    await page.setViewportSize({ width: 320, height: 900 });
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= window.innerWidth,
      ),
    ).toBe(true);
    await page.setViewportSize({ width: 1440, height: 900 });
    await field.focus();
    await field.blur();
    await selectValue(page, { label: "Режим сканирования", value: "fast" });
    const nextResolution = page.waitForRequest(
      (request) =>
        new URL(request.url()).pathname ===
          `/api/v1/servers/${serverId}/resolve` && request.method() === "POST",
    );
    const nextResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          `/api/v1/servers/${serverId}/jobs` &&
        response.request().method() === "POST",
    );
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    const next = decode("job", await (await nextResponse).json());
    expect(decode("retention", (await nextResolution).postDataJSON())).toEqual({
      kind: "before",
      local: exactLocal,
    });
    expect(next.intent.resolved_cutoff.local).toBe(exactLocal);
    expect(next.intent.resolved_cutoff.file_cutoff_utc).toBe(
      "2026-09-17T23:55:44.049364123Z",
    );
    expect(next.job_id).not.toBe(original.job_id);
    await page
      .getByRole("heading", { name: "Выполняется", exact: true })
      .waitFor();
    await page
      .getByRole("button", { name: "Отменить задание", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Отменено", exact: true })
      .waitFor();
    const cancelled = await fixture.stats();
    expect(cancelled.find((agent) => agent.id === serverId)?.puts).toBe(2);
    expect(cancelled.find((agent) => agent.id === serverId)?.cancels).toBe(1);
    expect(
      cancelled.find((agent) => agent.id === fixture.servers[1].id)?.puts,
    ).toBe(0);
    await field.fill("2026-09-19T03:20");
    await field.blur();
    const editedResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          `/api/v1/servers/${serverId}/jobs` &&
        response.request().method() === "POST",
    );
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    const edited = decode("job", await (await editedResponse).json());
    expect(edited.intent.resolved_cutoff.local).toBe("2026-09-19T03:20:00");
    expect(edited.intent.resolved_cutoff.file_cutoff_utc).toBe(
      "2026-09-19T00:20:00Z",
    );
    await page
      .getByRole("button", { name: "Отменить задание", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Отменено", exact: true })
      .waitFor();
  } finally {
    await fixture.close();
  }
}, 60000);

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
    await selectValue(page, { label: "Сервер", value: fixture.servers[0].id });
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
    await page.getByText("Дополнительные параметры", { exact: true }).click();
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
    await selectValue(page, { label: "Режим сканирования", value: "full" });
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
    await selectValue(page, { label: "Режим сканирования", value: "fast" });
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
  const { instrumentEvents } = await import("./instrumentation");
  const { decode } = await import("../../src/api");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await instrumentEvents(page);
    const replies: Array<
      Readonly<{
        at: number;
        path: string;
        status: number;
        phase: string | null;
        dispatch: string;
        slotHeld: boolean;
      }>
    > = [];
    const captures: Promise<void>[] = [];
    page.on("response", (response) => {
      const path = new URL(response.url()).pathname;
      if (!/\/jobs(?:\/[^/]+\/(?:reconcile|cancel))?$/.test(path)) return;
      const capture = response
        .json()
        .then((input: unknown) => {
          const job = decode("job", input);
          replies.push({
            at: Date.now(),
            path,
            status: response.status(),
            phase: job.snapshot?.phase ?? null,
            dispatch: job.dispatch,
            slotHeld: job.slot_held,
          });
        })
        .catch(() => undefined);
      captures.push(capture);
    });
    await signIn(page, fixture.origin);
    await fixture.loseResponse(fixture.servers[0].id, true);
    await fixture.hideLookup(fixture.servers[0].id, true);
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    try {
      await page
        .getByRole("heading", {
          name: "Проверка доставки задания",
          exact: true,
        })
        .waitFor();
    } catch (failure) {
      await Promise.all(captures);
      console.error(
        JSON.stringify({
          boundary: "unknown-delivery-fault",
          replies,
          stats: await fixture.stats(),
          receipt: await fixture.receipt(fixture.servers[0].id),
          events: await page.evaluate(() => window.__mailJobEvents),
          rendered: await page
            .getByRole("region", { name: "Состояние задания" })
            .innerText(),
        }),
      );
      throw failure;
    }
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
    const reconcileResponse = page.waitForResponse((response) =>
      new URL(response.url()).pathname.endsWith("/reconcile"),
    );
    await page
      .getByRole("button", { name: "Проверить состояние", exact: true })
      .click();
    const reconciled = decode("job", await (await reconcileResponse).json());
    expect(reconciled.dispatch).toBe("unknown");
    expect(reconciled.snapshot ?? null).toBeNull();
    expect(reconciled.slot_held).toBe(true);
    expect(
      await page
        .getByRole("heading", {
          name: "Проверка доставки задания",
          exact: true,
        })
        .isVisible(),
    ).toBe(true);
    const hidden = await fixture.receipt(fixture.servers[0].id);
    expect(hidden?.lookupHidden).toBe(true);
    expect(hidden?.lookups ?? 0).toBeGreaterThan(0);
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
    expect(
      (await fixture.stats()).find(
        (value) => value.id === fixture.servers[0].id,
      )?.cancels,
    ).toBe(1);
    expect((await fixture.receipt(fixture.servers[0].id))?.lookupHidden).toBe(
      true,
    );
    await fixture.hideLookup(fixture.servers[0].id, false);
    expect((await fixture.receipt(fixture.servers[0].id))?.lookupHidden).toBe(
      false,
    );
    await Promise.all(captures);
    console.info(
      JSON.stringify({
        boundary: "unknown-delivery-cancelled",
        replies,
        stats: await fixture.stats(),
        receipt: await fixture.receipt(fixture.servers[0].id),
        events: await page.evaluate(() => window.__mailJobEvents),
      }),
    );
  } finally {
    await fixture.close();
  }
}, 120000);

test("explicit receipt-fault release reconciles the accepted job without another dispatch", async () => {
  const { signIn } = await import("./cleanup-steps");
  const { decode } = await import("../../src/api");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await fixture.loseResponse(fixture.servers[0].id, true);
    await fixture.hideLookup(fixture.servers[0].id, true);
    const submissionResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          `/api/v1/servers/${fixture.servers[0].id}/jobs` &&
        response.request().method() === "POST",
    );
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    const submitted = decode("job", await (await submissionResponse).json());
    expect(submitted.dispatch).toBe("unknown");
    await page
      .getByRole("heading", { name: "Проверка доставки задания", exact: true })
      .waitFor();
    expect((await fixture.receipt(fixture.servers[0].id))?.lookupHidden).toBe(
      true,
    );
    await fixture.hideLookup(fixture.servers[0].id, false);
    const reconciliationResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
        `/api/v1/jobs/${submitted.job_id}/reconcile`,
    );
    await page
      .getByRole("button", { name: "Проверить состояние", exact: true })
      .click();
    const reconciled = decode(
      "job",
      await (await reconciliationResponse).json(),
    );
    expect(reconciled.job_id).toBe(submitted.job_id);
    expect(reconciled.dispatch).toBe("acknowledged");
    expect(reconciled.snapshot?.phase).toBe("running");
    await page
      .getByRole("heading", { name: "Выполняется", exact: true })
      .waitFor();
    expect(
      (await fixture.stats()).find(
        (agent) => agent.id === fixture.servers[0].id,
      )?.puts,
    ).toBe(1);
    await page
      .getByRole("button", { name: "Отменить задание", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Отменено", exact: true })
      .waitFor();
    const stats = await fixture.stats();
    expect(
      stats.find((agent) => agent.id === fixture.servers[0].id)?.puts,
    ).toBe(1);
    expect(
      stats.find((agent) => agent.id === fixture.servers[0].id)?.cancels,
    ).toBe(1);
    expect(
      stats.find((agent) => agent.id === fixture.servers[1].id)?.puts,
    ).toBe(0);
    console.info(
      JSON.stringify({
        boundary: "unknown-delivery-released",
        originalJobId: submitted.job_id,
        reconciledJobId: reconciled.job_id,
        phase: reconciled.snapshot?.phase,
        stats,
        receipt: await fixture.receipt(fixture.servers[0].id),
      }),
    );
  } finally {
    await fixture.close();
  }
}, 120000);

for (const operation of ["preview", "clean"] as const) {
  test(`same-server tabs adopt ${operation}, dismiss stale confirmation and cancel the exact job`, async () => {
    const { signIn, completedPreview } = await import("./cleanup-steps");
    const { instrumentEvents, sourceCounts } =
      await import("./instrumentation");
    const { decode } = await import("../../src/api");
    const fixture = await startFixture();
    try {
      const page = await fixture.context.newPage();
      await instrumentEvents(page);
      await signIn(page, fixture.origin);
      await page.getByText("Дополнительные параметры", { exact: true }).click();
      await page
        .getByLabel("Останавливать службу на время очистки", { exact: false })
        .check();
      const serverId = fixture.servers[0].id;
      await completedPreview(page, fixture, serverId);
      const other = await fixture.context.newPage();
      await instrumentEvents(other);
      await other.goto(`${fixture.origin}/?server=${serverId}`);
      await other
        .getByRole("heading", { name: "Завершено", exact: true })
        .waitFor();
      await other
        .getByRole("button", { name: "Очистить", exact: true })
        .click();
      await other.getByRole("dialog").waitFor();
      const submitted = page.waitForResponse(
        (response) =>
          new URL(response.url()).pathname ===
            `/api/v1/servers/${serverId}/jobs` &&
          response.request().method() === "POST",
      );
      if (operation === "preview") {
        await page
          .getByRole("button", { name: "Предпросмотр", exact: true })
          .click();
      } else {
        await page
          .getByRole("button", { name: "Очистить", exact: true })
          .click();
        await page
          .getByLabel(
            "Подтверждаю удаление и временную остановку почтовой службы.",
            { exact: true },
          )
          .check();
        await page
          .getByRole("button", { name: "Подтвердить очистку", exact: true })
          .click();
      }
      const next = decode("job", await (await submitted).json());
      await other
        .getByRole("heading", { name: "Выполняется", exact: true })
        .waitFor({ timeout: 10000 });
      expect(await other.getByRole("dialog").count()).toBe(0);
      expect(
        await other
          .getByRole("region", { name: "Состояние задания" })
          .locator("code")
          .textContent(),
      ).toBe(next.job_id);
      expect(
        await other
          .getByRole("button", { name: "Очистить", exact: true })
          .isDisabled(),
      ).toBe(true);
      const cancellation = other.waitForRequest(
        (request) =>
          new URL(request.url()).pathname ===
          `/api/v1/jobs/${next.job_id}/cancel`,
      );
      await other
        .getByRole("button", { name: "Отменить задание", exact: true })
        .click();
      await cancellation;
      if (operation === "clean") {
        await other
          .getByRole("heading", { name: "Восстановление службы", exact: true })
          .waitFor();
        await fixture.phase(serverId, "cancelled");
      }
      await other
        .getByRole("heading", { name: "Отменено", exact: true })
        .waitFor();
      await page
        .getByRole("heading", { name: "Отменено", exact: true })
        .waitFor();
      expect(await sourceCounts(page)).toEqual({ active: 1, created: 1 });
      expect(await sourceCounts(other)).toEqual({ active: 1, created: 1 });
      const stats = await fixture.stats();
      expect(stats.find((agent) => agent.id === serverId)?.puts).toBe(2);
      expect(stats.find((agent) => agent.id === serverId)?.cancels).toBe(1);
      expect(
        stats.find((agent) => agent.id === fixture.servers[1].id)?.puts,
      ).toBe(0);
    } finally {
      await fixture.close();
    }
  }, 60000);
}

test("Unicode login form submits all 64 supplementary scalars", async () => {
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await page.goto(`${fixture.origin}/login/`);
    const login = page.getByLabel("Логин", { exact: true });
    await login.fill("𐐀".repeat(63));
    await login.press("End");
    await page.keyboard.insertText("𐐀");
    expect(await login.inputValue()).toBe("𐐀".repeat(64));
    await page
      .getByLabel("Пароль", { exact: true })
      .fill("fixture-center-password");
    const submitted = page.waitForRequest(
      (request) => new URL(request.url()).pathname === "/api/v1/auth/login",
    );
    await page
      .getByRole("button", { name: "Войти в центр", exact: true })
      .click();
    expect((await submitted).postDataJSON().login).toBe("𐐀".repeat(64));
  } finally {
    await fixture.close();
  }
}, 60000);

test("delayed cleanup context cannot replace a newer terminal run or its confirmation", async () => {
  const { signIn, completedPreview } = await import("./cleanup-steps");
  const { decode } = await import("../../src/api");
  const fixture = await startFixture();
  const held = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  const finished = Promise.withResolvers<void>();
  try {
    const operator = await fixture.context.newPage();
    await signIn(operator, fixture.origin);
    const serverId = fixture.servers[0].id;
    await completedPreview(operator, fixture, serverId);
    const page = await fixture.context.newPage();
    await page.goto(`${fixture.origin}/?server=${serverId}`);
    await page
      .getByRole("heading", { name: "Завершено", exact: true })
      .waitFor();
    const old = decode(
      "cleanup_context",
      await page.evaluate(
        async (id) =>
          (await fetch(`/api/v1/servers/${id}/cleanup-context`)).json(),
        serverId,
      ),
    );
    let first = true;
    await page.route(
      `**/api/v1/servers/${serverId}/cleanup-context`,
      async (route) => {
        if (!first) {
          await route.continue();
          return;
        }
        first = false;
        held.resolve();
        await release.promise;
        try {
          await route.fulfill({ json: old });
        } finally {
          finished.resolve();
        }
      },
    );
    await page
      .getByRole("button", { name: "Обновить состояние", exact: true })
      .click();
    await held.promise;
    await operator
      .getByText("Дополнительные параметры", { exact: true })
      .click();
    await operator
      .getByLabel("Параллельные операции", { exact: true })
      .fill("3");
    await operator.getByLabel("Параллельные операции", { exact: true }).blur();
    const submitted = operator.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          `/api/v1/servers/${serverId}/jobs` &&
        response.request().method() === "POST",
    );
    await operator
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    const next = decode("job", await (await submitted).json());
    await fixture.finishLatest(serverId);
    await operator
      .getByRole("heading", { name: "Завершено", exact: true })
      .waitFor();
    await page
      .getByRole("button", { name: "Обновить состояние", exact: true })
      .click();
    await page
      .getByRole("region", { name: "Состояние задания" })
      .locator("code")
      .filter({ hasText: next.job_id })
      .waitFor();
    await page
      .getByRole("heading", { name: "Завершено", exact: true })
      .waitFor();
    expect(
      await page
        .getByLabel("Параллельные операции", { exact: true })
        .inputValue(),
    ).toBe("3");
    await page.getByRole("button", { name: "Очистить", exact: true }).click();
    await page.getByRole("dialog").waitFor();
    const staleResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          `/api/v1/servers/${serverId}/cleanup-context` &&
        response.status() === 200,
    );
    release.resolve();
    await finished.promise;
    await (await staleResponse).finished();
    await page.evaluate(
      () =>
        new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
    );
    expect(
      await page
        .getByRole("region", { name: "Состояние задания" })
        .locator("code")
        .textContent(),
    ).toBe(next.job_id);
    expect(
      await page
        .getByLabel("Параллельные операции", { exact: true })
        .inputValue(),
    ).toBe("3");
    expect(await page.getByRole("dialog").count()).toBe(1);
    await page.getByRole("button", { name: "Отмена", exact: true }).click();
    expect(
      await page
        .getByRole("button", { name: "Очистить", exact: true })
        .isEnabled(),
    ).toBe(true);
    const stats = await fixture.stats();
    expect(stats.find((agent) => agent.id === serverId)?.puts).toBe(2);
    expect(stats.every((agent) => agent.cancels === 0)).toBe(true);
  } finally {
    release.resolve();
    await fixture.close();
  }
}, 60000);

test("delayed cleanup context cannot revive a preview after owned dispatch without a newer context request", async () => {
  const { signIn } = await import("./cleanup-steps");
  const { decode } = await import("../../src/api");
  const fixture = await startFixture();
  const held = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  const finished = Promise.withResolvers<void>();
  try {
    const page = await fixture.context.newPage();
    // Transport fault: retain the one SSE connection, but drop snapshots so the
    // completed POST receipt is the sole advancement and cannot start a new GET.
    await page.addInitScript(() => {
      const Original = window.EventSource;
      window.EventSource = class extends Original {
        constructor(url: string | URL, options?: EventSourceInit) {
          super(url, options);
          this.addEventListener("snapshot", (event) =>
            event.stopImmediatePropagation(),
          );
        }
      };
    });
    await signIn(page, fixture.origin);
    await page.getByText("Дополнительные параметры", { exact: true }).click();
    await page
      .getByLabel("Останавливать службу на время очистки", { exact: false })
      .check();
    const serverId = fixture.servers[0].id;
    // Establish P via ordinary fixture-backed UI, then explicit authoritative GET.
    const previewResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          `/api/v1/servers/${serverId}/jobs` &&
        response.request().method() === "POST",
    );
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    const preview = decode("job", await (await previewResponse).json());
    await fixture.finishLatest(serverId);
    const operator = await fixture.context.newPage();
    await operator.goto(`${fixture.origin}/?server=${serverId}`);
    await operator
      .getByRole("heading", { name: "Завершено", exact: true })
      .waitFor();
    await page
      .getByRole("button", { name: "Обновить состояние", exact: true })
      .click();
    await page
      .getByRole("heading", { name: "Завершено", exact: true })
      .waitFor();
    const old = decode(
      "cleanup_context",
      await page.evaluate(
        async (id) =>
          (await fetch(`/api/v1/servers/${id}/cleanup-context`)).json(),
        serverId,
      ),
    );
    expect(old.latest?.job_id).toBe(preview.job_id);
    let contextRequests = 0;
    await page.route(
      `**/api/v1/servers/${serverId}/cleanup-context`,
      async (route) => {
        contextRequests++;
        held.resolve();
        await release.promise;
        try {
          await route.fulfill({ json: old });
        } finally {
          finished.resolve();
        }
      },
    );
    await page
      .getByRole("button", { name: "Обновить состояние", exact: true })
      .click();
    await held.promise;
    await page.route(`**/api/v1/servers/${serverId}/jobs`, async (route) => {
      const input = decode("submit", route.request().postDataJSON());
      // Real browser fetch preserves fixture TLS and the exact original command;
      // delay its successful receipt until this short run has already settled.
      await operator.evaluate(async (intent) => {
        const response = await fetch(
          `/api/v1/servers/${intent.server_id}/jobs`,
          {
            method: "POST",
            headers: {
              "Content-Type": "application/json",
              "X-HMail-Request": "1",
            },
            body: JSON.stringify(intent),
          },
        );
        if (!response.ok) throw new Error("Fixture clean was rejected");
        return response.json();
      }, input);
      await fixture.finishLatest(serverId);
      await operator
        .getByRole("region", { name: "Состояние задания" })
        .locator("code")
        .filter({ hasText: input.job_id })
        .waitFor();
      await operator
        .getByRole("heading", { name: "Завершено", exact: true })
        .waitFor();
      const latest = decode(
        "cleanup_context",
        await operator.evaluate(
          async (id) =>
            (await fetch(`/api/v1/servers/${id}/cleanup-context`)).json(),
          serverId,
        ),
      );
      if (
        !latest.latest ||
        latest.latest.job_id !== input.job_id ||
        latest.latest.slot_held
      )
        throw new Error("Completed command receipt unavailable");
      await route.fulfill({ json: latest.latest });
    });
    await page.getByRole("button", { name: "Очистить", exact: true }).click();
    await page
      .getByLabel(
        "Подтверждаю удаление и временную остановку почтовой службы.",
        { exact: true },
      )
      .check();
    const cleanResponse = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
          `/api/v1/servers/${serverId}/jobs` &&
        response.request().method() === "POST",
    );
    await page
      .getByRole("button", { name: "Подтвердить очистку", exact: true })
      .click();
    const clean = decode("job", await (await cleanResponse).json());
    await page
      .getByRole("region", { name: "Состояние задания" })
      .locator("code")
      .filter({ hasText: clean.job_id })
      .waitFor();
    await page
      .getByRole("heading", { name: "Завершено", exact: true })
      .waitFor();
    expect(contextRequests).toBe(1);
    release.resolve();
    await finished.promise;
    await page.evaluate(
      () =>
        new Promise<void>((resolve) => requestAnimationFrame(() => resolve())),
    );
    expect(
      await page
        .getByRole("region", { name: "Состояние задания" })
        .locator("code")
        .textContent(),
    ).toBe(clean.job_id);
    expect(
      await page
        .getByRole("button", { name: "Очистить", exact: true })
        .isDisabled(),
    ).toBe(true);
    expect(await page.getByRole("dialog").count()).toBe(0);
    expect(contextRequests).toBe(1);
    const stats = await fixture.stats();
    expect(stats.find((agent) => agent.id === serverId)?.puts).toBe(2);
    expect(stats.every((agent) => agent.cancels === 0)).toBe(true);
  } finally {
    release.resolve();
    await fixture.close();
  }
}, 60000);

test("passive SSE resync cannot silently consume an owned preview preflight", async () => {
  const { signIn } = await import("./cleanup-steps");
  const fixture = await startFixture();
  const held = Promise.withResolvers<void>();
  const release = Promise.withResolvers<void>();
  try {
    const page = await fixture.context.newPage();
    await page.addInitScript(() => {
      const Original = window.EventSource;
      window.EventSource = class extends Original {
        constructor(url: string | URL, options?: EventSourceInit) {
          super(url, options);
          document.addEventListener("fixture-context-resync", () =>
            this.dispatchEvent(new Event("resync")),
          );
        }
      };
    });
    await signIn(page, fixture.origin);
    const serverId = fixture.servers[0].id;
    const captured: unknown = await page.evaluate(
      async (id) =>
        (await fetch(`/api/v1/servers/${id}/cleanup-context`)).json(),
      serverId,
    );
    let requests = 0;
    await page.route(
      `**/api/v1/servers/${serverId}/cleanup-context`,
      async (route) => {
        requests++;
        if (requests === 1) {
          held.resolve();
          await release.promise;
        }
        await route.fulfill({ json: captured });
      },
    );
    await page
      .getByRole("button", { name: "Предпросмотр", exact: true })
      .click();
    await held.promise;
    const submitted = page
      .waitForResponse(
        (response) =>
          new URL(response.url()).pathname ===
            `/api/v1/servers/${serverId}/jobs` &&
          response.request().method() === "POST",
        { timeout: 5000 },
      )
      .then(
        (response) => response.json(),
        () => null,
      );
    await page.evaluate(() =>
      document.dispatchEvent(new Event("fixture-context-resync")),
    );
    release.resolve();
    expect(await submitted).not.toBeNull();
    await page
      .getByRole("heading", { name: "Выполняется", exact: true })
      .waitFor();
    expect(requests).toBe(1);
    expect(
      (await fixture.stats()).find((agent) => agent.id === serverId)?.puts,
    ).toBe(1);
    expect((await fixture.stats()).every((agent) => agent.cancels === 0)).toBe(
      true,
    );
  } finally {
    release.resolve();
    await fixture.close();
  }
}, 60000);
test("confirming the current retention option preserves the entered cutoff", async () => {
  const { signIn } = await import("./cleanup-steps");
  const fixture = await startFixture();
  try {
    const page = await fixture.context.newPage();
    await signIn(page, fixture.origin);
    await selectValue(page, { label: "Период хранения", value: "before" });
    const cutoff = page.getByRole("button", {
      name: "Граница по времени сервера",
      exact: true,
    });
    await selectDateTime(page, {
      label: "Граница по времени сервера",
      value: "2026-01-02T03:04:00",
    });
    const period = page.getByRole("combobox", {
      name: "Период хранения",
      exact: true,
    });
    await period.press("ArrowDown");
    await page
      .getByRole("listbox", { name: "Период хранения", exact: true })
      .waitFor();
    await period.press("Enter");
    expect(await cutoff.getAttribute("data-datetime-value")).toBe(
      "2026-01-02T03:04:00",
    );
    await selectValue(page, { label: "Период хранения", value: "before" });
    expect(await cutoff.getAttribute("data-datetime-value")).toBe(
      "2026-01-02T03:04:00",
    );
  } finally {
    await fixture.close();
  }
}, 120000);
