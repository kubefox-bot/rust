import compatibility from "../../deploy/compatibility.json";
export const PAYLOAD_MANIFEST = "payload-manifest.json";
export const APPLIANCE_MANIFEST = "appliance-manifest.json";
export const APPLIANCE_DISK = "astra-center.vhdx";
export const CHECKSUMS = "SHA256SUMS";
export const GUEST_PAYLOAD = "/var/lib/astra-appliance";
export const ROOT_LABEL = "root";
export const MAX_DOWNLOAD_BYTES = 2 * 1024 * 1024 * 1024;
export const DOWNLOAD_TIMEOUT_MS = 20 * 60 * 1000;
export const DISK_RESERVE_BYTES = 256 * 1024 * 1024;
export const MAX_RELEASE_ASSET_BYTES = 2 * 1024 * 1024 * 1024;
export const PAYLOAD_SCRIPT_FILES = [
  "prepare.sh",
  "prepare-user.sh",
  "initialize.sh",
  "lib/state.sh",
] as const;
export const RELEASE_PAYLOAD_FILES = [
  "center.oci.tar",
  "release-manifest.json",
  "quadlet/hmail-center.container",
  "quadlet/hmail-center-data.volume",
] as const;
export const BUNDLE_FILES = [
  APPLIANCE_DISK,
  "Install-Astra.ps1",
  "installer-lib.ps1",
  "kvpctl.exe",
  "base.ign",
  "README.txt",
  "assembly-report.json",
] as const;
export const SOURCE_SHA_PATTERN = /^[0-9a-f]{40}$/;
export const HASH_PATTERN = /^[0-9a-f]{64}$/;

const hash = { type: "string", pattern: "^[0-9a-f]{64}$" };
const sourceSha = { type: "string", pattern: "^[0-9a-f]{40}$" };
const https = { type: "string", pattern: "^https://[^\\s@]+$" };
const artifact = {
  type: "object",
  additionalProperties: false,
  required: ["name", "size", "sha256"],
  properties: {
    name: {
      type: "string",
      pattern: "^[A-Za-z0-9_+.-]+(?:/[A-Za-z0-9_+.-]+)*$",
    },
    size: { type: "integer", minimum: 1 },
    sha256: hash,
  },
};
export const LOCK_SCHEMA = {
  type: "object",
  additionalProperties: false,
  required: ["fcos", "kvpctl"],
  properties: {
    fcos: {
      type: "object",
      additionalProperties: false,
      required: ["release", "url", "sha256", "uncompressed_sha256"],
      properties: {
        release: { type: "string", pattern: "^[0-9.]+$" },
        url: https,
        sha256: hash,
        uncompressed_sha256: hash,
      },
    },
    kvpctl: {
      type: "object",
      additionalProperties: false,
      required: ["version", "url", "sha256"],
      properties: {
        version: { type: "string", pattern: "^v[0-9.]+$" },
        url: https,
        sha256: hash,
      },
    },
  },
};
const identity = {
  format_version: { const: 1 },
  source_sha: sourceSha,
  build: { type: "string", pattern: "^git-[0-9a-f]{40}$" },
  schema_version: { const: compatibility.current_schema_version },
};
export const PAYLOAD_SCHEMA = {
  type: "object",
  additionalProperties: false,
  required: [
    ...Object.keys(identity),
    "product",
    "image",
    "image_config_digest",
    "files",
  ],
  properties: {
    ...identity,
    product: { const: "astra-appliance-payload" },
    image: {
      type: "string",
      pattern: "^ghcr.io/kubefox-bot/astra-center@sha256:[0-9a-f]{64}$",
    },
    image_config_digest: { type: "string", pattern: "^sha256:[0-9a-f]{64}$" },
    files: { type: "array", minItems: 1, items: artifact },
  },
};
export const APPLIANCE_SCHEMA = {
  type: "object",
  additionalProperties: false,
  required: [
    ...Object.keys(identity),
    "product",
    "platform",
    "image",
    "upstream",
    "payload_manifest_sha256",
    "payload_checksums_sha256",
    "files",
  ],
  properties: {
    ...identity,
    product: { const: "astra-hyperv" },
    platform: { const: "linux/amd64" },
    image: {
      type: "object",
      required: [
        "reference",
        "digest",
        "config_digest",
        "local_tag",
        "platform",
      ],
      additionalProperties: false,
      properties: {
        reference: PAYLOAD_SCHEMA.properties.image,
        digest: PAYLOAD_SCHEMA.properties.image_config_digest,
        config_digest: PAYLOAD_SCHEMA.properties.image_config_digest,
        local_tag: { type: "string" },
        platform: { const: "linux/amd64" },
      },
    },
    upstream: LOCK_SCHEMA,
    payload_manifest_sha256: hash,
    payload_checksums_sha256: hash,
    files: { type: "array", minItems: 1, items: artifact },
  },
};
