import { join } from "node:path";
import { writeAtomic } from "./lib/filesystem";
import { createDeliveryDescriptor } from "./delivery-descriptor";
import { DELIVERY_BUNDLE, DELIVERY_FILE } from "./delivery-descriptor/conts";
import { readVerifiedRelease } from "./publishing/gate";
import { PROMOTION_FILE } from "./publishing/conts";
import { releaseAssets } from "./publishing/release";
import type { PublishInput, PublishedRelease } from "./publishing/types";

export const publishRelease = async ({
  directory,
  sourceSha,
  sequence,
  ports,
}: PublishInput): Promise<PublishedRelease> => {
  const { manifest, manifestSha256 } = await readVerifiedRelease({
    directory,
    sourceSha,
  });
  const descriptor = createDeliveryDescriptor({
    manifest,
    sourceSha,
    sequence,
    releaseManifestSha256: manifestSha256,
  });
  const bytes = JSON.stringify(descriptor, null, 2) + "\n";
  const draft = await ports.createDraft();
  if (draft.descriptor !== undefined && draft.descriptor !== bytes)
    throw new Error("Existing release sequence or image conflict");
  if (draft.published) {
    if (draft.descriptor === undefined)
      throw new Error("Published release is missing signed descriptor");
    await ports.verifyAssets(
      draft.id,
      releaseAssets({ directory, build: manifest.build }).map(
        (file) => file.name,
      ),
    );
    const result = {
      sourceSha,
      sequence,
      imageDigest: descriptor.image.digest,
      releaseId: draft.id,
    };
    await writeAtomic({
      path: join(directory, PROMOTION_FILE),
      value: JSON.stringify(result) + "\n",
      mode: 0o600,
    });
    return result;
  }
  const digest = await ports.copyImage();
  if (digest !== descriptor.image.digest)
    throw new Error("Registry transport changed verified image digest");
  const path = join(directory, DELIVERY_FILE);
  await writeAtomic({ path, value: bytes, mode: 0o600 });
  const bundle = await ports.sign(path);
  if (bundle !== join(directory, DELIVERY_BUNDLE))
    throw new Error("Unexpected delivery signing output");
  const assets = releaseAssets({ directory, build: manifest.build });
  await ports.upload(draft.id, assets);
  await ports.verifyAssets(
    draft.id,
    assets.map((file) => file.name),
  );
  const result = {
    sourceSha,
    sequence,
    imageDigest: descriptor.image.digest,
    releaseId: draft.id,
  };
  await writeAtomic({
    path: join(directory, PROMOTION_FILE),
    value: JSON.stringify(result) + "\n",
    mode: 0o600,
  });
  return result;
};
