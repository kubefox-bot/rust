import { join } from "node:path";
import { artifact } from "../manifest";
import { requireElfTarget } from "../elf";
import { ELF_HEADER_BYTES } from "../conts";
import { SOURCE_SHA_PATTERN } from "../delivery-descriptor/conts";
import { HOST_PACKAGE_MANIFEST } from "./conts";

export const validateUpdaterBinary = async (path: string): Promise<void> => {
  await artifact({ path, name: "astra-updater" });
  requireElfTarget({
    bytes: await Bun.file(path).slice(0, ELF_HEADER_BYTES).bytes(),
    platform: "linux/amd64",
  });
};
export const writePackageInventory = async ({
  directory,
  sourceSha,
  names,
}: Readonly<{
  directory: string;
  sourceSha: string;
  names: readonly string[];
}>): Promise<void> => {
  if (
    !SOURCE_SHA_PATTERN.test(sourceSha) ||
    names.length === 0 ||
    new Set(names).size !== names.length ||
    names.some(
      (name) =>
        !/^[a-zA-Z0-9._-]+$/.test(name) ||
        name === "." ||
        name === ".." ||
        name === HOST_PACKAGE_MANIFEST ||
        name === "SHA256SUMS",
    )
  )
    throw new Error("Unsafe package identity or payload name");
  const files = await Promise.all(
    [...names]
      .sort()
      .map((name) => artifact({ path: join(directory, name), name })),
  );
  const manifest = {
    format_version: 1,
    product: "astra-updater",
    version: 1,
    platform: "linux/amd64",
    build: `git-${sourceSha}`,
    source_sha: sourceSha,
    files,
  };
  const path = join(directory, HOST_PACKAGE_MANIFEST);
  await Bun.write(path, JSON.stringify(manifest, null, 2) + "\n");
  const manifestArtifact = await artifact({
    path,
    name: HOST_PACKAGE_MANIFEST,
  });
  await Bun.write(
    join(directory, "SHA256SUMS"),
    [...files, manifestArtifact]
      .map((file) => `${file.sha256}  ${file.name}\n`)
      .join(""),
  );
};
