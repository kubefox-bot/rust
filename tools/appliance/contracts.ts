import Ajv from "ajv";
import { APPLIANCE_SCHEMA, LOCK_SCHEMA, PAYLOAD_SCHEMA } from "./conts";
import type {
  ApplianceLock,
  ApplianceManifest,
  Artifact,
  PayloadManifest,
} from "./types";
const ajv = new Ajv({ strict: true });
const lock = ajv.compile<ApplianceLock>(LOCK_SCHEMA);
const payload = ajv.compile<PayloadManifest>(PAYLOAD_SCHEMA);
const appliance = ajv.compile<ApplianceManifest>(APPLIANCE_SCHEMA);
const requirePaths = (files: readonly Artifact[]): void => {
  if (
    new Set(files.map((file) => file.name)).size !== files.length ||
    files.some((file) =>
      file.name.split("/").some((part) => part === "." || part === ".."),
    )
  )
    throw new Error("Duplicate or unsafe appliance paths");
};
export const parseLock = (value: unknown): ApplianceLock => {
  if (!lock(value)) throw new Error("Invalid appliance upstream lock");
  if (
    !value.fcos.url.startsWith("https://builds.coreos.fedoraproject.org/") ||
    !value.kvpctl.url.startsWith(
      "https://github.com/containers/libhvee/releases/download/",
    )
  )
    throw new Error("Unexpected appliance upstream publisher");
  return value;
};
export const parsePayload = (value: unknown): PayloadManifest => {
  if (!payload(value) || value.build !== `git-${value.source_sha}`)
    throw new Error("Invalid appliance payload manifest");
  requirePaths(value.files);
  return value;
};
export const parseAppliance = (value: unknown): ApplianceManifest => {
  if (
    !appliance(value) ||
    value.build !== `git-${value.source_sha}` ||
    !value.image.reference.endsWith(`@${value.image.digest}`)
  )
    throw new Error("Invalid appliance manifest");
  requirePaths(value.files);
  parseLock(value.upstream);
  return value;
};
