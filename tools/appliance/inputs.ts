import { join } from "node:path";
import { run } from "../lib/process";
import { artifact, readReleaseManifest } from "../release/manifest";
import { sourceIdentity } from "../release/source-identity";
import { inspectOciArchive } from "../release/oci-archive";
import {
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_MANIFEST,
  HOST_PACKAGE_FILES,
  requireHostPackageIdentity,
} from "../release/updater-package/index";
import { requireRelativeFile, verifyInventory } from "./inventory";
import type { BuildOptions } from "./types";
import { RELEASE_PAYLOAD_FILES } from "./conts";
export const readVerifiedInputs = async (options: BuildOptions) => {
  const source = await sourceIdentity(options.root);
  const sha = (
    await run({ argv: ["git", "rev-parse", "HEAD"], cwd: options.root })
  ).trim();
  if (source.dirty || sha !== options.sourceSha)
    throw new Error("Appliance requires clean same-SHA source");
  const manifestFile = await artifact({
    path: await requireRelativeFile(
      options.releaseDirectory,
      "release-manifest.json",
    ),
    name: "release-manifest.json",
  });
  const release = await readReleaseManifest(
    join(options.releaseDirectory, "release-manifest.json"),
  );
  if (
    release.dirty ||
    release.build !== `git-${sha}` ||
    release.source_revision !== sha.slice(0, 12) ||
    release.source_fingerprint !== source.fingerprint ||
    release.image.platform !== "linux/amd64"
  )
    throw new Error("Appliance release differs from clean source or target");
  await verifyInventory({
    directory: options.releaseDirectory,
    files: release.artifacts,
  });
  const oci = await inspectOciArchive({
    path: await requireRelativeFile(options.releaseDirectory, "center.oci.tar"),
    build: release.build,
    revision: release.source_revision,
    platform: "linux/amd64",
    producer: release.engine,
  });
  if (
    oci.digest !== release.image.digest ||
    oci.configDigest !== release.image.config_digest
  )
    throw new Error("Appliance OCI bytes differ from release");
  const host = join(options.releaseDirectory, "host-updater");
  const hostFiles = await Promise.all(
    [HOST_PACKAGE_ARCHIVE, HOST_PACKAGE_MANIFEST].map(async (name) =>
      artifact({
        path: await requireRelativeFile(host, name),
        name: `host-updater/${name}`,
      }),
    ),
  );
  await requireHostPackageIdentity({ directory: host, sourceSha: sha });
  const members = (
    await run({
      argv: ["tar", "-tzf", join(host, HOST_PACKAGE_ARCHIVE)],
      cwd: options.root,
    })
  )
    .trim()
    .split("\n");
  const listing = await run({
    argv: ["tar", "-tvzf", join(host, HOST_PACKAGE_ARCHIVE)],
    cwd: options.root,
  });
  if (
    listing
      .trim()
      .split("\n")
      .some((line) => !line.startsWith("-")) ||
    members.length !== 11 ||
    new Set(members).size !== 11 ||
    JSON.stringify([...members].sort()) !==
      JSON.stringify(
        [...HOST_PACKAGE_FILES, HOST_PACKAGE_MANIFEST, "SHA256SUMS"].sort(),
      )
  )
    throw new Error("Unsafe updater package archive");

  const payloadNames: readonly string[] = RELEASE_PAYLOAD_FILES;
  const releaseFiles = release.artifacts.filter((file) =>
    payloadNames.includes(file.name),
  );
  if (releaseFiles.length !== RELEASE_PAYLOAD_FILES.length - 1)
    throw new Error("Release does not bind all payload artifacts");
  const files = [...releaseFiles, manifestFile, ...hostFiles];
  return { release, host, sourceSha: sha, files };
};
export const requireUnchangedSource = async ({
  root,
  sourceSha,
  sourceFingerprint,
}: Readonly<{
  root: string;
  sourceSha: string;
  sourceFingerprint: string;
}>): Promise<void> => {
  const source = await sourceIdentity(root);
  const sha = (
    await run({ argv: ["git", "rev-parse", "HEAD"], cwd: root })
  ).trim();
  if (
    source.dirty ||
    sha !== sourceSha ||
    source.fingerprint !== sourceFingerprint
  )
    throw new Error("Appliance source changed during assembly");
};
