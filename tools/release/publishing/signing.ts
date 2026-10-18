import { join } from "node:path";
import { run } from "../../lib/process";
import {
  DELIVERY_BUNDLE,
  SIGNER_IDENTITY,
  SIGNER_ISSUER,
} from "../delivery-descriptor/conts";
import type { PublicationConfig } from "./types";
import pins from "../../../deploy/tool-pins.json";
export const verifyDeliverySignature = async ({
  config,
  path,
  bundle,
  sourceSha,
}: Readonly<{
  config: PublicationConfig;
  path: string;
  bundle: string;
  sourceSha: string;
}>): Promise<void> => {
  if (
    Bun.SHA256.hash(await Bun.file(config.trustedRoot).bytes(), "hex") !==
    pins.trusted_root.sha256
  )
    throw new Error("Pinned trust root changed");
  await run({
    argv: [
      config.cosign,
      "verify-blob",
      "--bundle",
      bundle,
      "--trusted-root",
      config.trustedRoot,
      "--certificate-identity",
      SIGNER_IDENTITY,
      "--certificate-oidc-issuer",
      SIGNER_ISSUER,
      "--certificate-github-workflow-sha",
      sourceSha,
      path,
    ],
    cwd: config.root,
  });
};
export const signArtifact = async ({
  config,
  path,
  bundle,
}: Readonly<{
  config: PublicationConfig;
  path: string;
  bundle: string;
}>): Promise<string> => {
  await run({
    argv: [config.cosign, "sign-blob", "--yes", "--bundle", bundle, path],
    cwd: config.root,
  });
  await verifyDeliverySignature({
    config,
    path,
    bundle,
    sourceSha: config.sourceSha,
  });
  return bundle;
};

export const signDelivery = async ({
  config,
  path,
}: Readonly<{ config: PublicationConfig; path: string }>): Promise<string> =>
  signArtifact({
    config,
    path,
    bundle: join(config.directory, DELIVERY_BUNDLE),
  });
