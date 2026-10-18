import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import pins from "../../../deploy/appliance/inputs.json";
import { artifact } from "../../release/manifest";
import { run } from "../../lib/process";
import { APPLIANCE_MANIFEST, BUNDLE_FILES, CHECKSUMS } from "../conts";
import { inventoryFiles } from "../inventory";
import { checksums } from "../inventory";
import type { ReleaseManifest } from "../../release/types";
export const createBundleFixture = async ({
  directory,
  release,
  sourceSha,
}: Readonly<{
  directory: string;
  release: ReleaseManifest;
  sourceSha: string;
}>): Promise<void> => {
  await mkdir(directory);
  for (const name of BUNDLE_FILES)
    await Bun.write(join(directory, name), `fixture ${name}`);
  await Bun.write(
    join(directory, "base.ign"),
    JSON.stringify({ binding: `ASTRA_PAYLOAD_SHA256=${"e".repeat(64)}` }),
  );
  await Bun.write(
    join(directory, "assembly-report.json"),
    JSON.stringify({
      source_sha: sourceSha,
      build: release.build,
      checks: {
        offline_injection: "PASS",
        payload_readback: "PASS",
        upstream_unchanged: "PASS",
        first_boot_state: "PASS",
      },
    }),
  );
  await Bun.write(
    join(directory, APPLIANCE_MANIFEST),
    JSON.stringify({
      format_version: 1,
      product: "astra-hyperv",
      build: release.build,
      source_sha: sourceSha,
      platform: "linux/amd64",
      schema_version: 2,
      image: release.image,
      upstream: pins,
      payload_manifest_sha256: "f".repeat(64),
      payload_checksums_sha256: "e".repeat(64),
      files: await inventoryFiles({ directory, names: BUNDLE_FILES }),
    }),
  );
  const files = await inventoryFiles({
    directory,
    names: [...BUNDLE_FILES, APPLIANCE_MANIFEST],
  });
  await Bun.write(
    join(directory, CHECKSUMS),
    checksums([...files].sort((a, b) => a.name.localeCompare(b.name))),
  );
  await run({
    argv: [
      "zip",
      "-q",
      join(directory, `astra-hyperv-${release.build}.zip`),
      ...BUNDLE_FILES,
      APPLIANCE_MANIFEST,
      CHECKSUMS,
    ],
    cwd: directory,
  });
};
