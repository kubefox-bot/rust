import { readdir } from "node:fs/promises";
import { join } from "node:path";
import { artifact, readReleaseManifest } from "./manifest";
import { requireLifecycleReport } from "./publishing/gate";
import { SOURCE_SHA_PATTERN } from "./delivery-descriptor/conts";
import {
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_MANIFEST,
} from "./updater-package/conts";
import { CI_RECEIPT_FILE } from "./publishing/conts";
import { readReceiptConfig } from "./publishing/config";
import { applianceReleaseArtifacts } from "../appliance";

export const writeCiReceipt = async ({
  directory,
  sourceSha,
}: Readonly<{ directory: string; sourceSha: string }>): Promise<void> => {
  if (!SOURCE_SHA_PATTERN.test(sourceSha))
    throw new Error("Expected full CI source SHA");
  const manifestPath = join(directory, "release-manifest.json");
  const manifest = await readReleaseManifest(manifestPath);
  if (
    manifest.dirty ||
    manifest.build !== `git-${sourceSha}` ||
    manifest.source_revision !== sourceSha.slice(0, 12)
  )
    throw new Error("CI source identity mismatch");
  const reports = (await readdir(directory)).filter((name) =>
    /^delivery-[0-9a-f-]{36}$/.test(name),
  );
  if (reports.length !== 1)
    throw new Error(
      "Exactly one successful disposable delivery run is required",
    );
  const report: unknown = await Bun.file(
    join(directory, reports[0], "delivery-report.json"),
  ).json();
  requireLifecycleReport(report, manifest);
  const release = await artifact({
    path: manifestPath,
    name: "release-manifest.json",
  });
  const hostPackage = await artifact({
    path: join(directory, "host-updater", HOST_PACKAGE_ARCHIVE),
    name: HOST_PACKAGE_ARCHIVE,
  });
  const hostManifest = await artifact({
    path: join(directory, "host-updater", HOST_PACKAGE_MANIFEST),
    name: HOST_PACKAGE_MANIFEST,
  });
  await Bun.write(
    join(directory, CI_RECEIPT_FILE),
    JSON.stringify(
      {
        ...Object(report),
        source_sha: sourceSha,
        release_manifest_sha256: release.sha256,
        host_package_sha256: hostPackage.sha256,
        host_manifest_sha256: hostManifest.sha256,
        appliance_artifacts: await applianceReleaseArtifacts({ directory, sourceSha }),
      },
      null,
      2,
    ) + "\n",
  );
};
if (import.meta.main) await writeCiReceipt(readReceiptConfig());
