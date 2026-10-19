import { expect, test } from "bun:test";
import { mkdtemp, realpath, rm } from "node:fs/promises";
import { join, resolve } from "node:path";
import { tmpdir } from "node:os";

test("lost init response is recovered and a repeat never reinitializes the volume", async () => {
  const root = await realpath(await mkdtemp(join(tmpdir(), "astra-init-")));
  try {
    const script = resolve(
      import.meta.dir,
      "../../../deploy/appliance/initialize.sh",
    );
    const child = Bun.spawn(
      [
        "bash",
        "-c",
        [
          'set -euo pipefail; source "$1"; root=$2; mkdir -p "$root/state"',
          "sync() { return 0; }",
          'podman() { case " $* " in',
          '*" init-status "*) test -f "$root/committed" || return 3;;',
          '*" init "*) echo init >> "$root/calls"; touch "$root/committed"; return 1;;',
          "*) return 0;; esac; }",
          "systemctl() { return 0; }",
          'initialize_center "$root" "$root/state" https://localhost:8443 fixture fixture',
          'initialize_center "$root" "$root/state" https://localhost:8443 fixture fixture',
        ].join("\n"),
        "fixture",
        script,
        root,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    const code = await child.exited;
    const error = await new Response(child.stderr).text();
    expect(error).toBe("");
    expect(code).toBe(0);
    expect((await Bun.file(join(root, "calls")).text()).trim()).toBe("init");
    expect(await Bun.file(join(root, "state/ready")).exists()).toBe(true);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
test("an incomplete previous attempt is retained and not retried automatically", async () => {
  const root = await realpath(
    await mkdtemp(join(tmpdir(), "astra-incomplete-")),
  );
  try {
    const script = resolve(
      import.meta.dir,
      "../../../deploy/appliance/initialize.sh",
    );
    const child = Bun.spawn(
      [
        "bash",
        "-c",
        [
          'set -euo pipefail; source "$1"; root=$2; mkdir -p "$root/state"',
          "sync() { return 0; }",
          'write_state "$root/state" initialization.attempted fixture',
          'podman() { case " $* " in *" init-status "*) return 3;; *) echo changed >> "$root/calls"; return 0;; esac; }',
          'initialize_center "$root" "$root/state" https://localhost:8443 fixture fixture',
        ].join("\n"),
        "fixture",
        script,
        root,
      ],
      { stdout: "pipe", stderr: "pipe" },
    );
    expect(await child.exited).not.toBe(0);
    expect(await new Response(child.stderr).text()).toContain(
      "Previous initialization is incomplete",
    );
    expect(await Bun.file(join(root, "calls")).exists()).toBe(false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
