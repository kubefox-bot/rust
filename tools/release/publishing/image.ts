import { join } from "node:path";
import { run } from "../../lib/process";
import { inspectOciArchive } from "../oci-archive";
import { OCI_ARCHIVE } from "../conts";
import { SOURCE_LABEL } from "./conts";
import type { PublicationConfig } from "./types";
import type { ReleaseManifest } from "../types";

export const copyVerifiedImage = async ({
  config,
  manifest,
}: Readonly<{
  config: PublicationConfig;
  manifest: ReleaseManifest;
}>): Promise<string> => {
  const path = join(config.directory, OCI_ARCHIVE);
  const inspected = await inspectOciArchive({
    path,
    build: manifest.build,
    revision: manifest.source_revision,
    platform: manifest.image.platform,
    producer: manifest.engine,
  });
  if (
    inspected.digest !== manifest.image.digest ||
    inspected.configDigest !== manifest.image.config_digest
  )
    throw new Error("Verified OCI archive identity mismatch");
  const archive = new Bun.Archive(await Bun.file(path).bytes());
  const file = (
    await archive.files(`blobs/sha256/${inspected.configDigest.slice(7)}`)
  )
    .values()
    .next().value;
  if (!file) throw new Error("Missing OCI config");
  const image: unknown = await file.json();
  if (
    typeof image !== "object" ||
    image === null ||
    !("config" in image) ||
    typeof image.config !== "object" ||
    image.config === null ||
    !("Labels" in image.config) ||
    typeof image.config.Labels !== "object" ||
    image.config.Labels === null ||
    Reflect.get(image.config.Labels, SOURCE_LABEL) !== config.sourceSha
  )
    throw new Error("OCI image does not bind full source SHA");
  const tag = `ghcr.io/kubefox-bot/astra-center:git-${config.sourceSha}`;
  await run({
    argv: [
      "skopeo",
      "copy",
      "--preserve-digests",
      "--authfile",
      config.registryAuthFile,
      `oci-archive:${path}`,
      `docker://${tag}`,
    ],
    cwd: config.root,
  });
  const raw = await run({
    argv: [
      "skopeo",
      "inspect",
      "--raw",
      "--authfile",
      config.registryAuthFile,
      `docker://${manifest.image.reference}`,
    ],
    cwd: config.root,
  });
  const digest = `sha256:${Bun.SHA256.hash(raw, "hex")}`;
  if (digest !== manifest.image.digest)
    throw new Error("Registry changed OCI manifest bytes");
  const original = (await archive.files(`blobs/sha256/${digest.slice(7)}`))
    .values()
    .next().value;
  if (!original || (await original.text()) !== raw)
    throw new Error("Registry manifest, config or layers changed");
  return digest;
};
