import { mkdir } from "node:fs/promises";
import { join, dirname } from "node:path";
import { readReleaseConfig } from "./config";
import {
  AGENT_EXECUTABLE,
  OCI_ARCHIVE,
  PRIVATE_DIRECTORY_MODE,
  RELEASE_FORMAT_VERSION,
} from "./conts";
import { deployment } from "./deployment";
import {
  artifact,
  validateAgentPackage,
  writeReleaseManifest,
} from "./manifest";
import { inspectOciArchive } from "./oci-archive";
import { sourceIdentity } from "./source-identity";
import { buildAgentPackage } from "./build-agent";
import { validateBaseImages } from "./base-images";
import { buildImage } from "./build-image";
import type { ReleaseConfig } from "./types";

export const buildRelease = async (config: ReleaseConfig): Promise<void> => {
  await mkdir(dirname(config.directory), {
    recursive: true,
    mode: PRIVATE_DIRECTORY_MODE,
  });
  await mkdir(config.directory, { mode: PRIVATE_DIRECTORY_MODE });
  await validateBaseImages(config);
  const agentDirectory = await buildAgentPackage(config);
  const agent = await validateAgentPackage({
    directory: agentDirectory,
    build: config.build,
  });
  const archive = await buildImage(config, agentDirectory);
  const image = await inspectOciArchive({
    path: archive,
    build: config.build,
    revision: config.source.revision,
    platform: config.options.platform,
    producer: config.options.engine,
  });
  const reference = `${config.options.imageRepository}@${image.digest}`;
  const units = await deployment({ image: reference, build: config.build });
  const quadlet = join(config.directory, "quadlet");
  await mkdir(quadlet, { mode: PRIVATE_DIRECTORY_MODE });
  await Bun.write(join(quadlet, "hmail-center.container"), units.container);
  await Bun.write(join(quadlet, "hmail-center-data.volume"), units.volume);
  const relativeNames = [
    OCI_ARCHIVE,
    `agent-packages/${config.build}/${AGENT_EXECUTABLE}`,
    `agent-packages/${config.build}/${AGENT_EXECUTABLE}.sha256`,
    `agent-packages/${config.build}/manifest.json`,
    "quadlet/hmail-center.container",
    "quadlet/hmail-center-data.volume",
  ];
  const artifacts = await Promise.all(
    relativeNames.map((name) =>
      artifact({ path: join(config.directory, name), name }),
    ),
  );
  if (
    (await sourceIdentity(config.root)).fingerprint !==
    config.source.fingerprint
  )
    throw new Error(
      "Source changed during the release build; the incomplete output was retained for inspection",
    );
  await writeReleaseManifest({
    directory: config.directory,
    manifest: {
      format_version: RELEASE_FORMAT_VERSION,
      build: config.build,
      source_revision: config.source.revision,
      source_fingerprint: config.source.fingerprint,
      dirty: config.source.dirty,
      engine: config.options.engine,
      image: {
        reference,
        digest: image.digest,
        config_digest: image.configDigest,
        local_tag: config.imageTag,
        platform: config.options.platform,
      },
      base_images: config.bases,
      agent,
      artifacts,
    },
  });
  console.log(`Local release: ${config.directory}`);
  console.log(`Image: ${reference}`);
};

if (import.meta.main) await buildRelease(await readReleaseConfig());
