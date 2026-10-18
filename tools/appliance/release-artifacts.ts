import { join } from "node:path";
import { inventoryFiles } from "./inventory";
import { verifyBundle } from "./verify";
import { APPLIANCE_MANIFEST } from "./conts";
import type { Artifact } from "./types";
export const applianceReleaseArtifacts = async ({
  directory,
  sourceSha,
}: Readonly<{
  directory: string;
  sourceSha: string;
}>): Promise<readonly Artifact[]> => {
  const bundle = join(directory, "appliance");
  const manifest = await verifyBundle({ directory: bundle, sourceSha });
  const files = await inventoryFiles({
    directory: bundle,
    names: [
      APPLIANCE_MANIFEST,
      `astra-hyperv-${manifest.build}.zip`,
      "assembly-report.json",
    ],
  });
  return files.map((file) => ({ ...file, name: `appliance/${file.name}` }));
};
