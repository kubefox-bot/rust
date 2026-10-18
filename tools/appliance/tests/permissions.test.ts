import { expect, test } from "bun:test";
import { chmod, lstat, mkdir, mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { normalizePayload } from "../disk/archive";
test("private source modes and restrictive umask cannot hide immutable payload from UID2000", async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-modes-"));
  try {
    await mkdir(join(root, "quadlet"), { mode: 0o700 });
    await Bun.write(join(root, "quadlet/hmail-center.container"), "fixture");
    await chmod(join(root, "quadlet/hmail-center.container"), 0o600);
    await normalizePayload({
      payload: root,
      names: ["quadlet/hmail-center.container"],
    });
    expect((await lstat(root)).mode & 0o777).toBe(0o755);
    expect((await lstat(join(root, "quadlet"))).mode & 0o777).toBe(0o755);
    expect(
      (await lstat(join(root, "quadlet/hmail-center.container"))).mode & 0o777,
    ).toBe(0o644);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
