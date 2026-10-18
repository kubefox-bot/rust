import { join } from "node:path";
import { lstat } from "node:fs/promises";
import { artifact, readReleaseManifest } from "../manifest";
import {
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_MANIFEST,
} from "../updater-package/conts";
import { CI_RECEIPT_FILE, REQUIRED_CHECKS } from "./conts";
import type { ReleaseManifest } from "../types";

export const requireLifecycleReport = (
  value: unknown,
  manifest: ReleaseManifest,
): void => {
  if (
    typeof value !== "object" ||
    value === null ||
    !("build" in value) ||
    value.build !== manifest.build ||
    !("image_digest" in value) ||
    value.image_digest !== manifest.image.digest ||
    !("platform" in value) ||
    value.platform !== manifest.image.platform ||
    !("checks" in value) ||
    typeof value.checks !== "object" ||
    value.checks === null
  )
    throw new Error("CI lifecycle report does not match release");
  const checks: object = value.checks;
  for (const name of REQUIRED_CHECKS)
    if (!(name in checks) || Reflect.get(checks, name) !== "PASS")
      throw new Error(`CI lifecycle gate failed: ${name}`);
};
export const readVerifiedRelease = async ({
  directory,
  sourceSha,
}: Readonly<{ directory: string; sourceSha: string }>): Promise<
  Readonly<{ manifest: ReleaseManifest; manifestSha256: string }>
> => {
  const manifestPath = join(directory, "release-manifest.json");
  const manifest = await readReleaseManifest(manifestPath);
  const manifestArtifact = await artifact({
    path: manifestPath,
    name: "release-manifest.json",
  });
  const receipt: unknown = await Bun.file(
    join(directory, CI_RECEIPT_FILE),
  ).json();
  requireLifecycleReport(receipt, manifest);
  if (
    typeof receipt !== "object" ||
    receipt === null ||
    !("source_sha" in receipt) ||
    receipt.source_sha !== sourceSha ||
    !("release_manifest_sha256" in receipt) ||
    receipt.release_manifest_sha256 !== manifestArtifact.sha256
  )
    throw new Error("CI receipt does not bind the same source and manifest");
  const hostPackage = await artifact({
    path: join(directory, "host-updater", HOST_PACKAGE_ARCHIVE),
    name: HOST_PACKAGE_ARCHIVE,
  });
  const hostManifest = await artifact({
    path: join(directory, "host-updater", HOST_PACKAGE_MANIFEST),
    name: HOST_PACKAGE_MANIFEST,
  });
  if (
    !("host_package_sha256" in receipt) ||
    receipt.host_package_sha256 !== hostPackage.sha256 ||
    !("host_manifest_sha256" in receipt) ||
    receipt.host_manifest_sha256 !== hostManifest.sha256
  )
    throw new Error("CI host bootstrap package bytes changed");
  for (const expected of manifest.artifacts) {
    const segments = expected.name.split("/");
    if (
      expected.name.includes("\\") ||
      segments.some(
        (segment) =>
          segment === ".." || segment === "." || segment.length === 0,
      )
    )
      throw new Error("Unsafe release artifact path");
    for (let index = 1; index < segments.length; index++)
      if (
        !(
          await lstat(join(directory, ...segments.slice(0, index)))
        ).isDirectory()
      )
        throw new Error("Release artifacts must not traverse symlinks");
    const actual = await artifact({
      path: join(directory, expected.name),
      name: expected.name,
    });
    if (actual.sha256 !== expected.sha256 || actual.size !== expected.size)
      throw new Error("Verified release artifact changed");
  }
  return { manifest, manifestSha256: manifestArtifact.sha256 };
};
