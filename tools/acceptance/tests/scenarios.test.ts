import { expect, test } from "bun:test";
import { mkdtemp, mkdir, realpath, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { validateFixtureTarget } from "../fixture-target";
import { assertExecutionEvidence } from "../evidence";
test("acceptance refuses unmarked roots, production service names and escaped paths", async () => {
  const root = await mkdtemp(join(tmpdir(), "hmail-mvp-synthetic-"));
  try {
    const data = join(root, "mail");
    await mkdir(data);
    const target = {
      fixture_id: "fixture-one",
      root,
      data,
      service: "hmail-mvp-fixture-one",
    };
    await expect(validateFixtureTarget(target)).rejects.toThrow("marker");
    await Bun.write(
      join(root, ".hmail-mvp-fixture.json"),
      JSON.stringify({ fixture_id: "fixture-one", kind: "synthetic-only" }),
    );
    await expect(validateFixtureTarget(target)).resolves.toEqual({
      ...target,
      root: await realpath(root),
      data: await realpath(data),
    });
    await expect(
      validateFixtureTarget({ ...target, service: "hMailServer" }),
    ).rejects.toThrow();
    await expect(
      validateFixtureTarget({ ...target, data: join(root, "..") }),
    ).rejects.toThrow();
    const link = join(root, "outside");
    await symlink(tmpdir(), link);
    await expect(
      validateFixtureTarget({ ...target, data: link }),
    ).rejects.toThrow();
    await expect(
      validateFixtureTarget({ ...target, fixture_id: "fixture-other" }),
    ).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true });
  }
});
test("acceptance detects duplicate execution, unconfirmed targets, automatic clean and oversized bounded buffers", () => {
  const good = {
    confirmedServerId: "one",
    cleanedServerIds: ["one"],
    executions: [{ jobId: "j1", count: 1 }],
    maxJournalRows: 1000,
    maxQueuedEvents: 256,
    automaticCleanRequests: 0,
  };
  expect(() => assertExecutionEvidence(good)).not.toThrow();
  for (const changed of [
    { ...good, executions: [{ jobId: "j1", count: 2 }] },
    {
      ...good,
      executions: [
        { jobId: "j1", count: 1 },
        { jobId: "j1", count: 1 },
      ],
    },
    { ...good, cleanedServerIds: ["two"] },
    { ...good, maxJournalRows: 1001 },
    { ...good, maxQueuedEvents: 257 },
    { ...good, automaticCleanRequests: 1 },
  ])
    expect(() => assertExecutionEvidence(changed)).toThrow();
});
