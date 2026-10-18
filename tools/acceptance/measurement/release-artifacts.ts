import { dirname, join, resolve } from "node:path";
import { realpath } from "node:fs/promises";
import {
  artifact,
  readReleaseManifest,
  validateAgentPackage,
} from "../../release/manifest";
import type { MeasurementConfig } from "./types";
export const releaseArtifacts = async (config: MeasurementConfig) => {
  if (!config.releaseManifest)
    return {
      verdict: "NOT RUN",
      reason:
        "HMAIL_MVP_RELEASE_MANIFEST was not provided; OCI archive and Windows package size were not measured",
    };
  if ((await realpath(config.releaseManifest)) !== config.releaseManifest)
    throw new Error("Release manifest must not traverse a symlink");
  const manifest = await readReleaseManifest(config.releaseManifest);
  const directory = dirname(config.releaseManifest);
  const measured = [];
  for (const expected of manifest.artifacts) {
    const path = resolve(directory, expected.name);
    if (!path.startsWith(`${directory}/`) || (await realpath(path)) !== path)
      throw new Error(
        "Release artifact must remain within the exact release directory",
      );
    const actual = await artifact({ path, name: expected.name });
    if (actual.size !== expected.size || actual.sha256 !== expected.sha256)
      throw new Error(
        "Measured release artifact does not match the release manifest",
      );
    measured.push(actual);
  }
  await validateAgentPackage({
    directory: join(directory, "agent-packages", manifest.build),
    build: manifest.build,
  });
  return {
    verdict: "PASS",
    manifest: config.releaseManifest,
    build: manifest.build,
    image: manifest.image,
    artifacts: measured,
    semantics:
      "OCI archive file bytes and package/EXE bytes; not unpacked container storage, container RSS, or Windows runtime measurements",
  };
};
