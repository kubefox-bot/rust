import { expect, test } from "bun:test";
import { filesPath, clampOffset } from "../src/features/cleanup/files/query";
import { createPageLoader } from "../src/features/cleanup/files/loader";

test("file query encodes literal search and keeps uint64 paging exact", () => {
  expect(
    filesPath({
      jobId: "job-a",
      query: {
        search: "a%_\\ +",
        sort: "size",
        direction: "desc",
        offset: 9007199254741000n,
        limit: 50,
      },
    }),
  ).toBe(
    "/jobs/job-a/files?search=a%25_%5C+%2B&sort=size&direction=desc&offset=9007199254741000&limit=50",
  );
  expect(clampOffset({ offset: 150n, total: "101", limit: 50 })).toBe(100n);
  expect(clampOffset({ offset: 150n, total: "0", limit: 50 })).toBe(0n);
});

test("latest page aborts older work and discards late success or failure", async () => {
  const requests: {
    signal: AbortSignal;
    result: ReturnType<typeof Promise.withResolvers<string>>;
  }[] = [];
  const loader = createPageLoader(async (signal) => {
    const result = Promise.withResolvers<string>();
    requests.push({ signal, result });
    return result.promise;
  });
  const first = loader.load();
  const second = loader.load();
  expect(requests[0]?.signal.aborted).toBe(true);
  requests[1]?.result.resolve("current");
  const accepted = await second;
  expect(accepted?.page).toBe("current");
  expect(accepted?.current()).toBe(true);
  requests[0]?.result.resolve("stale");
  expect(await first).toBeNull();
  const closed = loader.load();
  expect(accepted?.current()).toBe(false);
  loader.cancel();
  requests[2]?.result.reject(new Error("late failure"));
  expect(await closed).toBeNull();
});

test("current page errors remain errors", async () => {
  const loader = createPageLoader(async () => {
    throw new Error("unsupported");
  });
  expect(loader.load()).rejects.toThrow("unsupported");
});

test("continuous invalidations refresh within one batch window", async () => {
  const { createRefreshScheduler } =
    await import("../src/features/cleanup/files/refresh");
  const events: string[] = [];
  const scheduler = createRefreshScheduler({
    refresh: async () => {
      events.push("refresh");
    },
    onerror: (cause) => {
      throw cause;
    },
    delayMs: 20,
  });
  scheduler.schedule();
  await Bun.sleep(10);
  scheduler.schedule();
  await Bun.sleep(15);
  expect(events).toEqual(["refresh"]);
  scheduler.schedule();
  scheduler.cancel();
  await Bun.sleep(25);
  expect(events).toEqual(["refresh"]);
});

test("the initial file revision does not schedule a duplicate page request", async () => {
  const { createRefreshScheduler } =
    await import("../src/features/cleanup/files/refresh");
  let requests = 0;
  const scheduler = createRefreshScheduler({
    refresh: async () => {
      requests++;
    },
    onerror: (cause) => {
      throw cause;
    },
    delayMs: 10,
  });
  scheduler.restart();
  scheduler.observe({
    revision: "1",
    refreshVersion: 0,
    terminal: false,
    visible: true,
  });
  await Bun.sleep(25);
  expect(requests).toBe(1);
  scheduler.observe({
    revision: "2",
    refreshVersion: 0,
    terminal: false,
    visible: true,
  });
  await Bun.sleep(15);
  expect(requests).toBe(2);
  scheduler.cancel();
});

test("a terminal revision queues an immediate follow-up without aborting the visible page", async () => {
  const { createRefreshScheduler } =
    await import("../src/features/cleanup/files/refresh");
  const requests: {
    signal: AbortSignal;
    result: ReturnType<typeof Promise.withResolvers<string>>;
  }[] = [];
  const visible: string[] = [];
  const loader = createPageLoader(async (signal) => {
    const result = Promise.withResolvers<string>();
    requests.push({ signal, result });
    return result.promise;
  });
  const scheduler = createRefreshScheduler({
    refresh: async () => {
      const loaded = await loader.load();
      if (loaded?.current()) visible.push(loaded.page);
    },
    onerror: (cause) => {
      throw cause;
    },
    delayMs: 100,
  });
  scheduler.restart();
  scheduler.observe({
    revision: "1",
    refreshVersion: 0,
    terminal: false,
    visible: true,
  });
  scheduler.observe({
    revision: "2",
    refreshVersion: 0,
    terminal: false,
    visible: true,
  });
  scheduler.observe({
    revision: "3",
    refreshVersion: 0,
    terminal: true,
    visible: true,
  });
  expect(requests).toHaveLength(1);
  expect(requests[0]?.signal.aborted).toBe(false);
  requests[0]?.result.resolve("scanning page");
  await Bun.sleep(0);
  expect(visible).toEqual(["scanning page"]);
  expect(requests).toHaveLength(2);
  requests[1]?.result.resolve("complete page");
  await Bun.sleep(0);
  expect(visible).toEqual(["scanning page", "complete page"]);
  scheduler.cancel();
  loader.cancel();
});

test("hidden file revisions remain idle and visible restart performs one current refresh", async () => {
  const { createRefreshScheduler } =
    await import("../src/features/cleanup/files/refresh");
  let requests = 0;
  const scheduler = createRefreshScheduler({
    refresh: async () => {
      requests++;
    },
    onerror: (cause) => {
      throw cause;
    },
    delayMs: 10,
  });
  scheduler.restart();
  scheduler.observe({
    revision: "1",
    refreshVersion: 0,
    terminal: false,
    visible: true,
  });
  scheduler.cancel();
  scheduler.observe({
    revision: "2",
    refreshVersion: 1,
    terminal: false,
    visible: false,
  });
  scheduler.observe({
    revision: "3",
    refreshVersion: 1,
    terminal: true,
    visible: false,
  });
  await Bun.sleep(25);
  expect(requests).toBe(1);
  scheduler.restart();
  scheduler.observe({
    revision: "3",
    refreshVersion: 1,
    terminal: true,
    visible: true,
  });
  await Bun.sleep(25);
  expect(requests).toBe(2);
  scheduler.cancel();
});

test("slow pages become visible during continuous invalidations with one follow-up", async () => {
  const { createRefreshScheduler } =
    await import("../src/features/cleanup/files/refresh");
  const requests: {
    signal: AbortSignal;
    result: ReturnType<typeof Promise.withResolvers<string>>;
  }[] = [];
  const visible: string[] = [];
  const loader = createPageLoader(async (signal) => {
    const result = Promise.withResolvers<string>();
    requests.push({ signal, result });
    return result.promise;
  });
  const scheduler = createRefreshScheduler({
    refresh: async () => {
      const loaded = await loader.load();
      if (loaded?.current()) visible.push(loaded.page);
    },
    onerror: (cause) => {
      throw cause;
    },
    delayMs: 10,
  });
  scheduler.restart();
  for (let index = 0; index < 5; index++) {
    scheduler.schedule();
    await Bun.sleep(10);
  }
  expect(requests.length).toBe(1);
  expect(requests[0]?.signal.aborted).toBe(false);
  requests[0]?.result.resolve("first page");
  await Bun.sleep(15);
  expect(visible).toEqual(["first page"]);
  expect(requests.length).toBe(2);
  requests[1]?.result.resolve("updated page");
  await Bun.sleep(20);
  expect(visible).toEqual(["first page", "updated page"]);
  expect(requests.length).toBe(2);
  scheduler.cancel();
  loader.cancel();
});

test("closing a dirty file request discards its page and queued refresh", async () => {
  const { createRefreshScheduler } =
    await import("../src/features/cleanup/files/refresh");
  const result = Promise.withResolvers<string>();
  const visible: string[] = [];
  let requests = 0;
  const loader = createPageLoader(async () => {
    requests++;
    return result.promise;
  });
  const scheduler = createRefreshScheduler({
    refresh: async () => {
      const loaded = await loader.load();
      if (loaded?.current()) visible.push(loaded.page);
    },
    onerror: (cause) => {
      throw cause;
    },
    delayMs: 10,
  });
  scheduler.restart();
  scheduler.schedule();
  scheduler.cancel();
  loader.cancel();
  result.resolve("closed page");
  await Bun.sleep(25);
  expect(visible).toEqual([]);
  expect(requests).toBe(1);
});

test("file filters serialize exact UTC bounds and linked false without losing search", () => {
  const path = filesPath({
    jobId: "job-a",
    query: {
      search: "mail",
      sort: "created_at",
      direction: "desc",
      offset: 0n,
      limit: 10,
      createdFrom: "2026-09-01T10:00:00.123Z",
      createdTo: "2026-09-02T10:00:00.000Z",
      orphan: false,
    },
  });
  const query = new URL(path, "https://localhost").searchParams;
  expect(query.get("created_from")).toBe("2026-09-01T10:00:00.123Z");
  expect(query.get("created_to")).toBe("2026-09-02T10:00:00.000Z");
  expect(query.get("orphan")).toBe("false");
  expect(query.get("search")).toBe("mail");
});

test("date filter conversion keeps local offset and rejects reversed ranges", async () => {
  const { resolveFilters } =
    await import("../src/features/cleanup/files/filters");
  const from = "2026-09-01T10:00:00.123456789";
  const result = resolveFilters({ from, to: from, relation: "linked" });
  expect(result.error).toBe("");
  expect(result.filters.orphan).toBe(false);
  expect(result.filters.createdFrom).toBe(
    `${new Date(from).toISOString().split(".")[0]}.123456789Z`,
  );
  expect(
    resolveFilters({
      from: "2026-09-02T00:00:00",
      to: "2026-09-01T00:00:00",
      relation: "all",
    }).error,
  ).not.toBe("");
  expect(resolveFilters({ from: "", to: "", relation: "all" }).filters).toEqual(
    {},
  );
});
