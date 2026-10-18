import { requireHostPackageIdentity } from "./host-package-identity";
import { join } from "node:path";
import { signArtifact, verifyDeliverySignature } from "./signing";
import { listAssets } from "./assets";
import {
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_BUNDLE,
} from "../updater-package/conts";
import type { PublicationConfig } from "./types";
import type { GithubClient } from "./github";

export const signHostPackage = async (
  config: PublicationConfig,
): Promise<void> => {
  const directory = join(config.directory, "host-updater");
  await requireHostPackageIdentity({ directory, sourceSha: config.sourceSha });
  await signArtifact({
    config,
    path: join(directory, HOST_PACKAGE_ARCHIVE),
    bundle: join(directory, HOST_PACKAGE_BUNDLE),
  });
};
export const restoreHostSignature = async ({
  client,
  config,
  releaseId,
}: Readonly<{
  client: GithubClient;
  config: PublicationConfig;
  releaseId: number;
}>): Promise<void> => {
  const assets = await listAssets(client, releaseId);
  const asset = assets.find((asset) => asset.name === HOST_PACKAGE_BUNDLE);
  if (!asset) throw new Error("Published bootstrap package lacks signature");
  const bundle = join(config.directory, "host-updater", HOST_PACKAGE_BUNDLE);
  await Bun.write(
    bundle,
    await client.text(
      await client.request({
        path: `/releases/assets/${asset.id}`,
        accept: "application/octet-stream",
      }),
    ),
  );
  await verifyDeliverySignature({
    config,
    path: join(config.directory, "host-updater", HOST_PACKAGE_ARCHIVE),
    bundle,
    sourceSha: config.sourceSha,
  });
};
