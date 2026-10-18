import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { artifact } from "../release/manifest";
import {
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_MANIFEST,
} from "../release/updater-package/index";
import compatibility from "../../deploy/compatibility.json";
import {
  PAYLOAD_MANIFEST,
  PAYLOAD_SCRIPT_FILES,
  RELEASE_PAYLOAD_FILES,
  CHECKSUMS,
} from "./conts";
import {
  inventoryFiles,
  listFiles,
  requireRelativeFile,
  verifyInventory,
} from "./inventory";
import { readVerifiedInputs } from "./inputs";
import { checksums } from "./inventory";
import { parsePayload } from "./contracts";
import type { BuildOptions, PayloadManifest } from "./types";
export const assemblePayload = async ({
  options,
  directory,
}: Readonly<{
  options: BuildOptions;
  directory: string;
}>): Promise<PayloadManifest> => {
  const {
    release,
    host,
    sourceSha: sha,
    files,
  } = await readVerifiedInputs(options);
  await mkdir(directory, { mode: 0o755 });
  const copy = async (from: string, name: string): Promise<void> => {
    await mkdir(dirname(join(directory, name)), {
      recursive: true,
      mode: 0o755,
    });
    await copyFile(from, join(directory, name));
  };
  for (const name of RELEASE_PAYLOAD_FILES)
    await copy(await requireRelativeFile(options.releaseDirectory, name), name);
  for (const name of [HOST_PACKAGE_ARCHIVE, HOST_PACKAGE_MANIFEST])
    await copy(await requireRelativeFile(host, name), `host-updater/${name}`);
  for (const name of PAYLOAD_SCRIPT_FILES)
    await copy(
      await requireRelativeFile(join(options.root, "deploy/appliance"), name),
      `scripts/${name}`,
    );
  for (const name of ["initialization.conf", "astra-initialize"])
    await copy(
      await requireRelativeFile(join(options.root, "deploy/appliance"), name),
      name,
    );
  await Bun.write(join(directory, "build"), `${release.build}\n`);
  await Bun.write(join(directory, "image"), `${release.image.reference}\n`);
  await Bun.write(
    join(directory, "image-config-digest"),
    `${release.image.config_digest}\n`,
  );
  await verifyInventory({ directory, files });
  const manifest = parsePayload({
    format_version: 1,
    product: "astra-appliance-payload",
    source_sha: sha,
    build: release.build,
    schema_version: compatibility.current_schema_version,
    image: release.image.reference,
    image_config_digest: release.image.config_digest,
    files: await inventoryFiles({
      directory,
      names: await listFiles(directory),
    }),
  });
  await Bun.write(
    join(directory, PAYLOAD_MANIFEST),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  await Bun.write(
    join(directory, CHECKSUMS),
    checksums(
      [
        ...manifest.files,
        await artifact({
          path: join(directory, PAYLOAD_MANIFEST),
          name: PAYLOAD_MANIFEST,
        }),
      ].sort((a, b) => a.name.localeCompare(b.name)),
    ),
  );
  return manifest;
};
