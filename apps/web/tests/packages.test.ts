import { expect, test } from "bun:test";
import { createPackageCatalog } from "../src/agents/catalog";
import type { BrowserContracts } from "../src/api";

test("one page shares pending and completed package reads", async () => {
  const result = Promise.withResolvers<BrowserContracts["packages"]>();
  let requests = 0;
  const catalog = createPackageCatalog(() => {
    requests++;
    return result.promise;
  });
  const footer = catalog.read();
  const agents = catalog.read();
  expect(requests).toBe(1);
  result.resolve([]);
  expect(await footer).toEqual([]);
  expect(await agents).toEqual([]);
  expect(await catalog.read()).toEqual([]);
  expect(requests).toBe(1);
});

test("failed catalog reads can retry and another session has its own cache", async () => {
  let requests = 0;
  const read = async (): Promise<BrowserContracts["packages"]> => {
    requests++;
    if (requests === 1) throw new Error("unavailable");
    return [];
  };
  const first = createPackageCatalog(read);
  expect(await first.read().catch((error: unknown) => error)).toEqual(
    new Error("unavailable"),
  );
  expect(await first.read()).toEqual([]);
  expect(await first.read()).toEqual([]);
  const second = createPackageCatalog(read);
  expect(await second.read()).toEqual([]);
  expect(requests).toBe(3);
});
