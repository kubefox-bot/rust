import { resolve } from "node:path";
import { SOURCE_SHA_PATTERN } from "../delivery-descriptor/conts";
import type { PublicationConfig } from "./types";
import type { ReleaseEnvironment } from "../types";

export const readReceiptConfig = (
  environment: ReleaseEnvironment = Bun.env,
): Readonly<{ directory: string; sourceSha: string }> => {
  const directory = environment.HMAIL_DELIVERY_DIRECTORY;
  const sourceSha = environment.HMAIL_DELIVERY_SOURCE_SHA;
  if (!directory || !sourceSha || !SOURCE_SHA_PATTERN.test(sourceSha))
    throw new Error("Delivery directory and full source SHA are required");
  return { directory: resolve(directory), sourceSha };
};
export const readPublicationConfig = (
  environment: ReleaseEnvironment = Bun.env,
  args: readonly string[] = Bun.argv.slice(2),
): PublicationConfig => {
  if (args.length !== 1 || (args[0] !== "publish" && args[0] !== "promote"))
    throw new Error("Expected publish or promote");
  const receipt = readReceiptConfig(environment);
  const sequence = Number(environment.HMAIL_DELIVERY_SEQUENCE);
  const token = environment.GH_TOKEN;
  const registryAuthFile = environment.HMAIL_REGISTRY_AUTH_FILE;
  if (
    !Number.isSafeInteger(sequence) ||
    sequence < 1 ||
    !token ||
    !registryAuthFile
  )
    throw new Error(
      "Positive release sequence, GH_TOKEN and registry auth file are required",
    );
  return {
    ...receipt,
    sequence,
    token,
    registryAuthFile: resolve(registryAuthFile),
    root: resolve(import.meta.dir, "../../.."),
    trustedRoot: resolve(
      import.meta.dir,
      "../../../deploy/trust/sigstore-trusted-root.json",
    ),
    cosign: environment.HMAIL_COSIGN_PATH ?? "cosign",
    mode: args[0],
  };
};
