import { expect, test } from "bun:test";
import { mkdtemp, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
test("a foreign volume is never adopted or removed", async () => {
  const root = await realpath(await mkdtemp(join(tmpdir(), "astra-volume-")));
  try {
    const child = Bun.spawn(
      [
        "bash",
        "-c",
        'source "$1"; ensure_state "$2"; podman() { return 0; }; claim_volume "$2" fixture',
        "fixture",
        resolve(import.meta.dir, "../../../deploy/appliance/prepare-user.sh"),
        root,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    expect(await child.exited).not.toBe(0);
    expect(await new Response(child.stderr).text()).toContain(
      "existing unclaimed",
    );
    expect(await Bun.file(join(root, "volume.claim")).exists()).toBe(false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
