import { expect, test } from "bun:test";
import { mkdtemp, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
test("a probe transport/storage failure never becomes an uninitialized volume", async () => {
  const root = await realpath(await mkdtemp(join(tmpdir(), "astra-probe-")));
  try {
    const child = Bun.spawn(
      [
        "bash",
        "-c",
        [
          'source "$1"; root=$2; sync() { return 0; }',
          'podman() { case " $* " in *" init-status "*) return 125;; *) touch "$root/changed";; esac; }',
          'initialize_center "$root" "$root/state" https://localhost:8443 fixture fixture',
        ].join("\n"),
        "fixture",
        resolve(import.meta.dir, "../../../deploy/appliance/initialize.sh"),
        root,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    const [code, error] = await Promise.all([
      child.exited,
      new Response(child.stderr).text(),
    ]);
    expect(code).not.toBe(0);
    expect(error).toContain("no init was attempted");
    expect(await Bun.file(join(root, "changed")).exists()).toBe(false);
    expect(
      await Bun.file(join(root, "state/initialization.attempted")).exists(),
    ).toBe(false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
