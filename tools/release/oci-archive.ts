import {
  CENTER_EXECUTABLE,
  DISALLOWED_RUNTIME_FILES,
  IMAGE_DIGEST_PATTERN,
  MAX_OCI_LAYER_BYTES,
  MAX_OCI_METADATA_BYTES,
  IMAGE_HEALTH_REQUIRED,
  ELF_FILE,
  ELF_HEADER_BYTES,
} from "./conts";
import type { Engine, Platform } from "./types";
import { requireDirectHealthcheck } from "./healthcheck";
import { requireElfTarget } from "./elf";

const descriptorDigest = (value: unknown): string => {
  if (
    typeof value !== "object" ||
    value === null ||
    !("digest" in value) ||
    typeof value.digest !== "string" ||
    !IMAGE_DIGEST_PATTERN.test(value.digest)
  )
    throw new Error("Invalid OCI content descriptor");
  return value.digest;
};
const selectedFile = async (
  archive: Bun.Archive,
  name: string,
  maxBytes: number,
): Promise<File> => {
  const file = (await archive.files(name)).get(name);
  if (!file || file.size === 0 || file.size > maxBytes)
    throw new Error("Missing or oversized OCI metadata or layer");
  return file;
};
const blob = async (
  archive: Bun.Archive,
  digest: string,
  maxBytes: number,
): Promise<File> => {
  const file = await selectedFile(
    archive,
    `blobs/sha256/${digest.slice("sha256:".length)}`,
    maxBytes,
  );
  const hash = new Bun.CryptoHasher("sha256");
  for await (const bytes of file.stream()) hash.update(bytes);
  if (`sha256:${hash.digest("hex")}` !== digest)
    throw new Error("OCI content digest mismatch");
  return file;
};
const runtimeConfig = (
  value: unknown,
  expected: Readonly<{
    build: string;
    revision: string;
    platform: Platform;
    producer: Engine;
  }>,
) => {
  if (
    typeof value !== "object" ||
    value === null ||
    !("architecture" in value) ||
    value.architecture !== expected.platform.split("/")[1] ||
    !("os" in value) ||
    value.os !== "linux" ||
    !("config" in value) ||
    typeof value.config !== "object" ||
    value.config === null
  )
    throw new Error("OCI platform does not match the selected release");
  const config = value.config;
  if (
    !("User" in config) ||
    config.User !== "10001:10001" ||
    !("Entrypoint" in config) ||
    !Array.isArray(config.Entrypoint) ||
    config.Entrypoint.length !== 1 ||
    config.Entrypoint[0] !== CENTER_EXECUTABLE ||
    !("Labels" in config) ||
    typeof config.Labels !== "object" ||
    config.Labels === null ||
    !("org.opencontainers.image.version" in config.Labels) ||
    config.Labels["org.opencontainers.image.version"] !== expected.build ||
    !("org.opencontainers.image.revision" in config.Labels) ||
    config.Labels["org.opencontainers.image.revision"] !== expected.revision
  )
    throw new Error(
      "OCI runtime ownership, entrypoint or build identity does not match the release",
    );
  if (!("Healthcheck" in config) && IMAGE_HEALTH_REQUIRED[expected.producer])
    throw new Error("Runtime image requires a direct healthcheck");
  if ("Healthcheck" in config)
    requireDirectHealthcheck({ value: config.Healthcheck });
};
export const inspectOciArchive = async ({
  path,
  build,
  revision,
  platform,
  producer,
}: Readonly<{
  path: string;
  build: string;
  revision: string;
  platform: Platform;
  producer: Engine;
}>): Promise<Readonly<{ digest: string; configDigest: string }>> => {
  const archive = new Bun.Archive(await Bun.file(path).bytes());
  const layout: unknown = await (
    await selectedFile(archive, "oci-layout", MAX_OCI_METADATA_BYTES)
  ).json();
  if (
    typeof layout !== "object" ||
    layout === null ||
    !("imageLayoutVersion" in layout) ||
    layout.imageLayoutVersion !== "1.0.0"
  )
    throw new Error("Expected OCI image layout v1");
  const index: unknown = await (
    await selectedFile(archive, "index.json", MAX_OCI_METADATA_BYTES)
  ).json();
  if (
    typeof index !== "object" ||
    index === null ||
    !("manifests" in index) ||
    !Array.isArray(index.manifests) ||
    index.manifests.length !== 1
  )
    throw new Error(
      "Release archive must select exactly one platform manifest",
    );
  const entries: readonly unknown[] = index.manifests;
  const digest = descriptorDigest(entries[0]);
  const manifest: unknown = await (
    await blob(archive, digest, MAX_OCI_METADATA_BYTES)
  ).json();
  if (
    typeof manifest !== "object" ||
    manifest === null ||
    !("schemaVersion" in manifest) ||
    manifest.schemaVersion !== 2 ||
    !("config" in manifest) ||
    !("layers" in manifest) ||
    !Array.isArray(manifest.layers)
  )
    throw new Error("Invalid OCI image manifest");
  const configDigest = descriptorDigest(manifest.config);
  const config: unknown = await (
    await blob(archive, configDigest, MAX_OCI_METADATA_BYTES)
  ).json();
  runtimeConfig(config, { build, revision, platform, producer });
  const layers: readonly unknown[] = manifest.layers;
  let executable: Uint8Array | undefined = undefined;
  for (const layer of layers) {
    const contents = new Bun.Archive(
      await (
        await blob(archive, descriptorDigest(layer), MAX_OCI_LAYER_BYTES)
      ).arrayBuffer(),
    );
    if ((await contents.files(DISALLOWED_RUNTIME_FILES)).size !== 0)
      throw new Error(
        "Runtime image contains a JavaScript runtime, compiler or shell",
      );
    const center = (await contents.files(ELF_FILE)).get(ELF_FILE);
    if (center) executable = await center.slice(0, ELF_HEADER_BYTES).bytes();
  }
  if (!executable)
    throw new Error("Runtime image is missing its center ELF executable");
  requireElfTarget({ bytes: executable, platform });
  return { digest, configDigest };
};
