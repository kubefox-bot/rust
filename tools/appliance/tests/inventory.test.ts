import { expect, test } from "bun:test";
import { mkdtemp, mkdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { inventoryFiles, verifyInventory } from "../inventory";

test("inventory detects changed bytes and refuses symlinked or escaping paths", async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-payload-"));
  try {
    await mkdir(join(root, "release"));
    await Bun.write(join(root, "release/file"), "approved bytes");
    const files = await inventoryFiles({
      directory: root,
      names: ["release/file"],
    });
    await verifyInventory({ directory: root, files });
    await Bun.write(join(root, "release/file"), "changed bytes");
    await expect(verifyInventory({ directory: root, files })).rejects.toThrow();
    await symlink(join(root, "release/file"), join(root, "link"));
    await expect(
      inventoryFiles({ directory: root, names: ["link"] }),
    ).rejects.toThrow();
    await expect(
      inventoryFiles({ directory: root, names: ["../outside"] }),
    ).rejects.toThrow();
    await expect(
      inventoryFiles({
        directory: root,
        names: ["release/file", "release/file"],
      }),
    ).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
