import { expect, test } from "bun:test";
import { mkdtemp, mkdir, rm, symlink } from "node:fs/promises";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { PAYLOAD_SCRIPT_FILES, RELEASE_PAYLOAD_FILES } from "../conts";
test("bootstrap rejects changed payload, unlisted files and symlinks before execution", async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-payload-"));
  try {
    const names = [
      ...RELEASE_PAYLOAD_FILES,
      "host-updater/astra-updater-linux-amd64.tar.gz",
      "host-updater/package-manifest.json",
      ...PAYLOAD_SCRIPT_FILES.map((name) => `scripts/${name}`),
      "initialization.conf",
      "astra-initialize",
      "image",
      "build",
      "image-config-digest",
      "payload-manifest.json",
    ];
    for (const name of names) {
      await mkdir(dirname(join(root, name)), { recursive: true });
      await Bun.write(join(root, name), "fixture");
    }
    const sums = names
      .map((name) => `${Bun.SHA256.hash("fixture", "hex")}  ${name}\n`)
      .join("");
    await Bun.write(join(root, "SHA256SUMS"), sums);
    const verify = async () => {
      const child = Bun.spawn(
        [
          "bash",
          "-c",
          'source "$1"; verify_payload "$2" "$3"',
          "fixture",
          resolve(import.meta.dir, "../../../deploy/appliance/prepare.sh"),
          root,
          Bun.SHA256.hash(sums, "hex"),
        ],
        { stdout: "pipe", stderr: "pipe" },
      );
      const [code] = await Promise.all([
        child.exited,
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
      ]);
      return code;
    };
    expect(await verify()).toBe(0);
    await Bun.write(join(root, "center.oci.tar"), "changed");
    expect(await verify()).not.toBe(0);
    await Bun.write(join(root, "center.oci.tar"), "fixture");
    await Bun.write(join(root, "secret"), "unlisted");
    expect(await verify()).not.toBe(0);
    await rm(join(root, "secret"));
    await rm(join(root, "image"));
    await symlink(join(root, "build"), join(root, "image"));
    expect(await verify()).not.toBe(0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
