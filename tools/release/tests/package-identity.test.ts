import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  HOST_PACKAGE_FILES,
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_MANIFEST,
  requireHostPackageIdentity,
} from "../updater-package/index";
import { writePackageInventory } from "../updater-package/inventory";
test("host package identity verifies all archived bytes before bootstrap", async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-package-identity-"));
  try {
    const elf = new Uint8Array(64);
    elf.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1]);
    new DataView(elf.buffer).setUint16(18, 62, true);
    for (const name of HOST_PACKAGE_FILES)
      await Bun.write(
        join(directory, name),
        name === "astra-updater" ? elf : "fixture",
      );
    const sourceSha = "a".repeat(40);
    await writePackageInventory({
      directory,
      sourceSha,
      names: HOST_PACKAGE_FILES,
    });
    const archive = async () => {
      const entries = Object.fromEntries(
        await Promise.all(
          [...HOST_PACKAGE_FILES, HOST_PACKAGE_MANIFEST, "SHA256SUMS"].map(
            async (name) => [
              name,
              await Bun.file(join(directory, name)).bytes(),
            ],
          ),
        ),
      );
      await Bun.write(
        join(directory, HOST_PACKAGE_ARCHIVE),
        await new Bun.Archive(entries).bytes(),
      );
    };
    await archive();
    await requireHostPackageIdentity({ directory, sourceSha });
    await Bun.write(join(directory, "install.sh"), "altered");
    await archive();
    await expect(
      requireHostPackageIdentity({ directory, sourceSha }),
    ).rejects.toThrow("differs");
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
