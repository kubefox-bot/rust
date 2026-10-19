import { expect, test } from "bun:test";
import { mkdtemp, rm, symlink } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import {
  validateUpdaterBinary,
  writePackageInventory,
} from "../updater-package/inventory";

test("updater package inventory binds all payload bytes, common build and safe relative names", async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-package-"));
  try {
    await Bun.write(join(directory, "fixture.txt"), "fixture bytes");
    await writePackageInventory({
      directory,
      sourceSha: "a".repeat(40),
      names: ["fixture.txt"],
    });
    const manifest = await Bun.file(
      join(directory, "package-manifest.json"),
    ).json();
    expect(manifest).toMatchObject({
      product: "astra-updater",
      version: 1,
      build: `git-${"a".repeat(40)}`,
      platform: "linux/amd64",
    });
    expect(manifest.files[0].sha256).toBe(
      Bun.SHA256.hash("fixture bytes", "hex"),
    );
    const checksums = await Bun.file(join(directory, "SHA256SUMS")).text();
    expect(checksums).toContain(`${manifest.files[0].sha256}  fixture.txt`);
    expect(checksums).toContain("  package-manifest.json");
    await expect(
      writePackageInventory({
        directory,
        sourceSha: "a".repeat(40),
        names: ["../escape"],
      }),
    ).rejects.toThrow();
    await symlink("fixture.txt", join(directory, "alias.txt"));
    await expect(
      writePackageInventory({
        directory,
        sourceSha: "a".repeat(40),
        names: ["alias.txt"],
      }),
    ).rejects.toThrow();
  } finally {
    await rm(directory, { recursive: true });
  }
});
test("host updater binary must be Linux amd64 ELF, not native Mac or Windows", async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-updater-elf-"));
  try {
    const path = join(directory, "updater");
    const bytes = new Uint8Array(64);
    bytes.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1]);
    new DataView(bytes.buffer).setUint16(18, 62, true);
    await Bun.write(path, bytes);
    await expect(validateUpdaterBinary(path)).resolves.toBeUndefined();
    new DataView(bytes.buffer).setUint16(18, 183, true);
    await Bun.write(path, bytes);
    await expect(validateUpdaterBinary(path)).rejects.toThrow("architecture");
    await Bun.write(path, "MZfake");
    await expect(validateUpdaterBinary(path)).rejects.toThrow("ELF");
  } finally {
    await rm(directory, { recursive: true });
  }
});
