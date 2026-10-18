import { expect, test } from "bun:test";
import { verifyBundle } from "../verify";
import { createBundleFixture } from "../testing/bundle";
import {
  releaseFixture,
  sourceSha,
} from "../../release/tests/delivery-fixture";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
test("missing appliance receipt/assets fail closed", async () => {
  await expect(
    verifyBundle({
      directory: "/does-not-exist/astra",
      sourceSha: "a".repeat(40),
    }),
  ).rejects.toThrow();
});
test("bundle verification reads ZIP members and rejects changed bytes and source", async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-bundle-"));
  const directory = join(root, "bundle");
  try {
    await createBundleFixture({
      directory,
      release: releaseFixture(),
      sourceSha,
    });
    expect((await verifyBundle({ directory, sourceSha })).build).toBe(
      `git-${sourceSha}`,
    );
    await expect(
      verifyBundle({ directory, sourceSha: "b".repeat(40) }),
    ).rejects.toThrow("source");
    await Bun.write(join(directory, "astra-center.vhdx"), "modified disk");
    await expect(verifyBundle({ directory, sourceSha })).rejects.toThrow(
      "changed",
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
