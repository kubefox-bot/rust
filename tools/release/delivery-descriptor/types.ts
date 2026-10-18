import type { ReleaseManifest } from "../types";
export type DeliveryDescriptor = Readonly<{
  format_version: 1;
  product: "astra-center";
  channel: "main";
  source_sha: string;
  build: string;
  sequence: number;
  image: Readonly<{
    reference: string;
    digest: string;
    config_digest: string;
    platform: "linux/amd64";
  }>;
  release_manifest_sha256: string;
  updater_min: number;
  updater_max: number;
  schema_version: number;
  protocol_min: number;
  protocol_max: number;
}>;
export type DeliveryDescriptorInput = Readonly<{
  manifest: ReleaseManifest;
  sourceSha: string;
  sequence: number;
  releaseManifestSha256: string;
}>;
