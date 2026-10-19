import Ajv from "ajv";
import schema from "../../deploy/delivery.schema.json";
import {
  DELIVERY_CHANNEL,
  DELIVERY_FORMAT,
  DELIVERY_PRODUCT,
  MAX_DESCRIPTOR_BYTES,
  SOURCE_SHA_PATTERN,
  UPDATER_VERSION,
  SCHEMA_VERSION,
} from "./delivery-descriptor/conts";
import { PROTOCOL_MIN, PROTOCOL_MAX } from "./conts";
import type {
  DeliveryDescriptor,
  DeliveryDescriptorInput,
} from "./delivery-descriptor/types";
export type {
  DeliveryDescriptor,
  DeliveryDescriptorInput,
} from "./delivery-descriptor/types";

const validate = new Ajv({ strict: true }).compile<DeliveryDescriptor>(schema);
export const parseDeliveryDescriptor = (bytes: string): DeliveryDescriptor => {
  if (Buffer.byteLength(bytes) > MAX_DESCRIPTOR_BYTES)
    throw new Error("Oversized delivery descriptor");
  const value: unknown = JSON.parse(bytes);
  if (!validate(value)) throw new Error("Invalid delivery descriptor contract");
  if (
    value.build !== `git-${value.source_sha}` ||
    value.image.reference !==
      `ghcr.io/kubefox-bot/astra-center@${value.image.digest}` ||
    value.updater_min > value.updater_max ||
    value.protocol_min > value.protocol_max
  )
    throw new Error("Inconsistent delivery identities or compatibility bounds");
  return value;
};
export const createDeliveryDescriptor = ({
  manifest,
  sourceSha,
  sequence,
  releaseManifestSha256,
}: DeliveryDescriptorInput): DeliveryDescriptor => {
  if (
    !SOURCE_SHA_PATTERN.test(sourceSha) ||
    manifest.dirty ||
    manifest.source_revision !== sourceSha.slice(0, 12) ||
    manifest.build !== `git-${sourceSha}` ||
    manifest.agent.build !== manifest.build ||
    manifest.image.platform !== "linux/amd64"
  )
    throw new Error("Delivery requires a clean, same-SHA Linux amd64 release");
  return parseDeliveryDescriptor(
    JSON.stringify({
      format_version: DELIVERY_FORMAT,
      product: DELIVERY_PRODUCT,
      channel: DELIVERY_CHANNEL,
      source_sha: sourceSha,
      build: manifest.build,
      sequence,
      image: manifest.image && {
        reference: manifest.image.reference,
        digest: manifest.image.digest,
        config_digest: manifest.image.config_digest,
        platform: manifest.image.platform,
      },
      release_manifest_sha256: releaseManifestSha256,
      updater_min: UPDATER_VERSION,
      updater_max: UPDATER_VERSION,
      schema_version: SCHEMA_VERSION,
      protocol_min: PROTOCOL_MIN,
      protocol_max: PROTOCOL_MAX,
    }),
  );
};
