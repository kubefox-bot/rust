import { join } from "node:path";
import { HOST_PACKAGE_ARCHIVE, HOST_PACKAGE_MANIFEST } from "./conts";
import { requireElfTarget } from "../elf";
import { ELF_HEADER_BYTES } from "../conts";
import { verifyArchiveInventory } from "./archive";
export const requireHostPackageIdentity = async ({
  directory,
  sourceSha,
}: Readonly<{ directory: string; sourceSha: string }>): Promise<void> => {
  const archive = new Bun.Archive(
    await Bun.file(join(directory, HOST_PACKAGE_ARCHIVE)).bytes(),
  );
  const entries = await archive.files([HOST_PACKAGE_MANIFEST, "astra-updater"]);
  const manifest = entries.get(HOST_PACKAGE_MANIFEST);
  const binary = entries.get("astra-updater");
  if (!manifest || !binary) throw new Error("Bootstrap package is incomplete");
  if (
    (await manifest.text()) !==
    (await Bun.file(join(directory, HOST_PACKAGE_MANIFEST)).text())
  )
    throw new Error("Bootstrap package manifest differs from archive");
  const identity: unknown = await manifest.json();
  if (
    typeof identity !== "object" ||
    identity === null ||
    !("format_version" in identity) ||
    identity.format_version !== 1 ||
    !("product" in identity) ||
    identity.product !== "astra-updater" ||
    !("version" in identity) ||
    identity.version !== 1 ||
    !("build" in identity) ||
    identity.build !== `git-${sourceSha}` ||
    !("source_sha" in identity) ||
    identity.source_sha !== sourceSha ||
    !("platform" in identity) ||
    identity.platform !== "linux/amd64"
  )
    throw new Error("Bootstrap package source or platform identity mismatch");
  requireElfTarget({
    bytes: await binary.slice(0, ELF_HEADER_BYTES).bytes(),
    platform: "linux/amd64",
  });
  await verifyArchiveInventory({
    archive,
    files: "files" in identity ? identity.files : undefined,
  });
};
