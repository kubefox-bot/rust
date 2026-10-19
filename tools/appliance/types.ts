import type { Artifact, ReleaseManifest } from "../release/types";
export type { Artifact };
export type PayloadManifest = Readonly<{
  format_version: 1;
  product: "astra-appliance-payload";
  source_sha: string;
  build: string;
  image: string;
  image_config_digest: string;
  schema_version: number;
  files: readonly Artifact[];
}>;
export type ApplianceLock = Readonly<{
  fcos: Readonly<{
    release: string;
    url: string;
    sha256: string;
    uncompressed_sha256: string;
  }>;
  kvpctl: Readonly<{ version: string; url: string; sha256: string }>;
}>;
export type ApplianceManifest = Readonly<{
  format_version: 1;
  product: "astra-hyperv";
  build: string;
  source_sha: string;
  platform: "linux/amd64";
  schema_version: number;
  image: ReleaseManifest["image"];
  upstream: ApplianceLock;
  payload_manifest_sha256: string;
  payload_checksums_sha256: string;
  files: readonly Artifact[];
}>;
export type BuildOptions = Readonly<{
  root: string;
  releaseDirectory: string;
  output: string;
  sourceSha: string;
  cache: string;
}>;
