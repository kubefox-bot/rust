import { join } from "node:path";
import { readVerificationConfig } from "./config";
import { OCI_ARCHIVE } from "./conts";
import { verifyLifecycle } from "./delivery/lifecycle";
import {
  artifact,
  readReleaseManifest,
  validateAgentPackage,
} from "./manifest";
import { inspectOciArchive } from "./oci-archive";

if (import.meta.main) {
  const config = readVerificationConfig();
  const manifest = await readReleaseManifest(config.manifestPath);
  for (const expected of manifest.artifacts) {
    const actual = await artifact({
      path: join(config.directory, expected.name),
      name: expected.name,
    });
    if (actual.sha256 !== expected.sha256 || actual.size !== expected.size)
      throw new Error("Release artifact checksum or size mismatch");
  }
  await validateAgentPackage({
    directory: join(config.directory, "agent-packages", manifest.build),
    build: manifest.build,
  });
  const image = await inspectOciArchive({
    path: join(config.directory, OCI_ARCHIVE),
    build: manifest.build,
    revision: manifest.source_revision,
    platform: manifest.image.platform,
    producer: manifest.engine,
  });
  if (
    image.digest !== manifest.image.digest ||
    image.configDigest !== manifest.image.config_digest
  )
    throw new Error("OCI image identity mismatch");
  await verifyLifecycle({ config, manifest });
}
