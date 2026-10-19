import { lstat } from "node:fs/promises";
import { writeAtomic } from "./lib/filesystem";
import { join } from "node:path";
import {
  AGENT_EXECUTABLE,
  AGENT_PRODUCT,
  AGENT_TARGET,
  IMAGE_DIGEST_PATTERN,
  MANIFEST_FILE_MODE,
  PROTOCOL_MAX,
  PROTOCOL_MIN,
  RELEASE_MANIFEST,
} from "./conts";
import type { AgentManifest, Artifact, ReleaseManifest } from "./types";
import Ajv from "ajv";
import { RELEASE_SCHEMA } from "./release-manifest/conts";
import { pinnedImage, configuredBuildVersion } from "./config";

const validateRelease = new Ajv({ strict: true }).compile<ReleaseManifest>(
  RELEASE_SCHEMA,
);
export const readReleaseManifest = async (
  path: string,
): Promise<ReleaseManifest> => {
  const value: unknown = await Bun.file(path).json();
  if (!validateRelease(value))
    throw new Error("Invalid release manifest contract");
  configuredBuildVersion({ HMAIL_BUILD_VERSION: value.build });
  if (
    value.agent.build !== value.build ||
    !value.image.reference.endsWith(`@${value.image.digest}`)
  )
    throw new Error(
      "Release manifest contains inconsistent build or image identities",
    );
  for (const base of Object.values(value.base_images)) pinnedImage(base);
  const names = value.artifacts.map((entry) => entry.name);
  if (new Set(names).size !== names.length)
    throw new Error("Duplicate release artifacts");
  return value;
};

export const artifact = async ({
  path,
  name,
}: Readonly<{ path: string; name: string }>): Promise<Artifact> => {
  const metadata = await lstat(path);
  if (
    !metadata.isFile() ||
    metadata.size === 0 ||
    !Number.isSafeInteger(metadata.size)
  )
    throw new Error("Release artifact must be a nonempty regular file");
  if (name.startsWith("/") || name.split(/[\\/]/).includes(".."))
    throw new Error("Artifact name must remain within its release");
  const hash = new Bun.CryptoHasher("sha256");
  for await (const chunk of Bun.file(path).stream()) hash.update(chunk);
  return { name, size: metadata.size, sha256: hash.digest("hex") };
};
export const validateAgentPackage = async ({
  directory,
  build,
}: Readonly<{ directory: string; build: string }>): Promise<AgentManifest> => {
  const value: unknown = await Bun.file(
    join(directory, "manifest.json"),
  ).json();
  if (
    typeof value !== "object" ||
    value === null ||
    !("product" in value) ||
    value.product !== AGENT_PRODUCT ||
    !("target" in value) ||
    value.target !== AGENT_TARGET ||
    !("build" in value) ||
    value.build !== build ||
    !("protocol_min" in value) ||
    value.protocol_min !== PROTOCOL_MIN ||
    !("protocol_max" in value) ||
    value.protocol_max !== PROTOCOL_MAX ||
    !("sha256" in value) ||
    typeof value.sha256 !== "string" ||
    !IMAGE_DIGEST_PATTERN.test(`sha256:${value.sha256}`)
  )
    throw new Error(
      "Agent package does not match the common release build and protocol",
    );
  const executable = await artifact({
    path: join(directory, AGENT_EXECUTABLE),
    name: AGENT_EXECUTABLE,
  });
  if (executable.sha256 !== value.sha256)
    throw new Error("Agent executable does not match its manifest");
  return {
    product: AGENT_PRODUCT,
    target: AGENT_TARGET,
    build,
    protocol_min: PROTOCOL_MIN,
    protocol_max: PROTOCOL_MAX,
    sha256: executable.sha256,
  };
};
export const writeReleaseManifest = async ({
  directory,
  manifest,
}: Readonly<{
  directory: string;
  manifest: ReleaseManifest;
}>): Promise<void> => {
  await writeAtomic({
    path: join(directory, RELEASE_MANIFEST),
    value: `${JSON.stringify(manifest, null, 2)}\n`,
    mode: MANIFEST_FILE_MODE,
  });
};
