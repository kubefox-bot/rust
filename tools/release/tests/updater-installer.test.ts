import { expect, test } from "bun:test";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  rm,
  stat,
  symlink,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const installer = resolve(
  import.meta.dir,
  "../../../deploy/updater/install.sh",
);
const files = [
  "astra-updater",
  "cosign",
  "sigstore-trusted-root.json",
  "install.sh",
  "config.example.json",
  "package-manifest.json",
  "astra-updater.service",
  "astra-updater.timer",
  "astra-updater-recover.service",
  "astra-reboot-coordinator.service",
] as const;
const run = async (args: readonly string[]) => {
  const child = Bun.spawn(
    [
      "bash",
      "-c",
      'set -euo pipefail; source "$1"; shift; "$@"',
      "fixture",
      installer,
      ...args,
    ],
    { stdout: "pipe", stderr: "pipe" },
  );
  const [code, output, error] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
  ]);
  return { code, output, error };
};
const fixture = async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-installer-"));
  const directory = join(root, "package");
  await mkdir(directory);
  const checksums: string[] = [];
  for (const file of files) {
    const content = `fixture ${file}\n`;
    await writeFile(join(directory, file), content);
    checksums.push(`${Bun.SHA256.hash(content, "hex")}  ${file}`);
  }
  await writeFile(join(directory, "SHA256SUMS"), `${checksums.join("\n")}\n`);
  return { root, directory };
};

test("offline installer verifies every approved package file and rejects changed bytes", async () => {
  const f = await fixture();
  try {
    expect((await run(["verify_package", f.directory])).code).toBe(0);
    await writeFile(join(f.directory, "astra-updater"), "tampered");
    expect((await run(["verify_package", f.directory])).code).not.toBe(0);
  } finally {
    await rm(f.root, { recursive: true, force: true });
  }
});

test("installer rejects traversal, duplicate checksum names, missing files and symlinks", async () => {
  const f = await fixture();
  try {
    const manifest = await readFile(join(f.directory, "SHA256SUMS"), "utf8");
    for (const contents of [
      `${"a".repeat(64)}  ../outside\n`,
      `${manifest}${manifest.split("\n")[0]}\n`,
      manifest.split("\n").slice(1).join("\n"),
    ]) {
      await writeFile(join(f.directory, "SHA256SUMS"), contents);
      expect((await run(["verify_package", f.directory])).code).not.toBe(0);
    }
    await writeFile(join(f.directory, "SHA256SUMS"), manifest);
    await rm(join(f.directory, "cosign"));
    await symlink(
      join(f.directory, "astra-updater"),
      join(f.directory, "cosign"),
    );
    expect((await run(["verify_package", f.directory])).code).not.toBe(0);
  } finally {
    await rm(f.root, { recursive: true, force: true });
  }
});

test("initial installer creates private config and identity and refuses to overwrite them", async () => {
  const f = await fixture();
  const home = join(f.root, "home");
  try {
    await mkdir(home);
    const identity = "11111111-1111-4111-8111-111111111111";
    const first = await run(["install_files", f.directory, home, identity]);
    expect(first.code).toBe(0);
    const config = join(home, ".config/astra-updater/config.json");
    const client = join(home, ".local/state/astra-updater/reboot-client.json");
    expect(JSON.parse(await readFile(client, "utf8"))).toEqual({
      id: identity,
      group: "astra",
    });
    expect((await stat(config)).mode & 0o777).toBe(0o600);
    expect((await stat(join(home, ".config/astra-updater"))).mode & 0o777).toBe(
      0o700,
    );
    expect(
      (await stat(join(home, ".local/bin/astra-updater"))).mode & 0o777,
    ).toBe(0o755);
    expect(
      await readFile(
        join(home, ".config/astra-updater/credentials.json"),
        "utf8",
      ),
    ).toBe("{}\n");
    await writeFile(config, "preserve operator config");
    expect(
      (await run(["install_files", f.directory, home, identity])).code,
    ).not.toBe(0);
    expect(await readFile(config, "utf8")).toBe("preserve operator config");
    expect(
      await readFile(
        join(
          home,
          ".config/systemd/user/hmail-center.service.d/20-astra-recovery.conf",
        ),
        "utf8",
      ),
    ).toContain("Requires=astra-updater-recover.service");
  } finally {
    await rm(f.root, { recursive: true, force: true });
  }
});

test("installer refuses a symlinked configuration directory before copying any binary", async () => {
  const f = await fixture();
  const home = join(f.root, "home");
  const external = join(f.root, "outside");
  try {
    await mkdir(join(home, ".config"), { recursive: true });
    await mkdir(external);
    await symlink(external, join(home, ".config/astra-updater"));
    expect(
      (
        await run([
          "install_files",
          f.directory,
          home,
          "11111111-1111-4111-8111-111111111111",
        ])
      ).code,
    ).not.toBe(0);
    expect(
      await Bun.file(join(home, ".local/bin/astra-updater")).exists(),
    ).toBe(false);
    expect(await Bun.file(join(external, "config.json")).exists()).toBe(false);
  } finally {
    await rm(f.root, { recursive: true, force: true });
  }
});
