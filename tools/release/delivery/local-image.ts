import type { ReleaseManifest } from "../types";

export const localImageId = ({
  value,
  expected,
}: Readonly<{
  value: unknown;
  expected: Pick<ReleaseManifest, "build" | "source_revision" | "image">;
}>): string => {
  if (!Array.isArray(value) || value.length !== 1)
    throw new Error("Local Docker image does not match the verified release");
  const entries: readonly unknown[] = value;
  const image = entries[0];
  if (
    typeof image !== "object" ||
    image === null ||
    !("Id" in image) ||
    typeof image.Id !== "string" ||
    (image.Id !== expected.image.digest &&
      image.Id !== expected.image.config_digest) ||
    !("Os" in image) ||
    image.Os !== "linux" ||
    !("Architecture" in image) ||
    image.Architecture !== expected.image.platform.split("/")[1] ||
    !("Config" in image) ||
    typeof image.Config !== "object" ||
    image.Config === null ||
    !("Labels" in image.Config) ||
    typeof image.Config.Labels !== "object" ||
    image.Config.Labels === null
  )
    throw new Error("Local Docker image does not match the verified release");
  const labels = image.Config.Labels;
  if (
    !("org.opencontainers.image.version" in labels) ||
    labels["org.opencontainers.image.version"] !== expected.build ||
    !("org.opencontainers.image.revision" in labels) ||
    labels["org.opencontainers.image.revision"] !== expected.source_revision
  )
    throw new Error("Local Docker image does not match the verified release");
  return image.Id;
};
