import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { parseDeliveryDescriptor } from "../delivery-descriptor";
import type { DeliveryDescriptor } from "../delivery-descriptor/types";
import { DELIVERY_BUNDLE, DELIVERY_FILE } from "../delivery-descriptor/conts";
import { MAX_RELEASE_PAGES } from "./conts";
import { listAssets } from "./assets";
import { verifyDeliverySignature } from "./signing";
import type { GithubClient } from "./github";
import type { PublicationConfig } from "./types";

export const readSignedDescriptor = async ({
  client,
  config,
  releaseId,
  retainDirectory,
}: Readonly<{
  client: GithubClient;
  config: PublicationConfig;
  releaseId: number;
  retainDirectory?: string;
}>): Promise<string | undefined> => {
  const assets = await listAssets(client, releaseId);
  const descriptor = assets.find((asset) => asset.name === DELIVERY_FILE);
  if (!descriptor) return undefined;
  const bundle = assets.find((asset) => asset.name === DELIVERY_BUNDLE);
  if (!bundle) throw new Error("Published descriptor lacks signature bundle");
  // GitHub API asset download redirects to an expiring signed HTTPS URL. Credentials
  // are sent only to api.github.com; the redirected CDN request has no Authorization.
  const download = async (id: number): Promise<string> => {
    const response = await client.request({
      path: `/releases/assets/${id}`,
      accept: "application/octet-stream",
    });
    return client.text(response);
  };
  const bytes = await download(descriptor.id);
  const parsed = parseDeliveryDescriptor(bytes);
  const directory = await mkdtemp(join(tmpdir(), "astra-published-signature-"));
  try {
    const path = join(directory, DELIVERY_FILE);
    const bundlePath = join(directory, DELIVERY_BUNDLE);
    await Bun.write(path, bytes);
    await Bun.write(bundlePath, await download(bundle.id));
    await verifyDeliverySignature({
      config,
      path,
      bundle: bundlePath,
      sourceSha: parsed.source_sha,
    });
    if (retainDirectory) {
      await Bun.write(join(retainDirectory, DELIVERY_FILE), bytes);
      await Bun.write(
        join(retainDirectory, DELIVERY_BUNDLE),
        await Bun.file(bundlePath).bytes(),
      );
    }
    return bytes;
  } finally {
    await rm(directory, { recursive: true });
  }
};
export const publishedDescriptors = async ({
  client,
  config,
}: Readonly<{ client: GithubClient; config: PublicationConfig }>): Promise<
  readonly DeliveryDescriptor[]
> => {
  const descriptors: DeliveryDescriptor[] = [];
  for (let page = 1; page <= MAX_RELEASE_PAGES; page++) {
    const releases = await client.json({
      path: `/releases?per_page=100&page=${page}`,
    });
    if (!Array.isArray(releases))
      throw new Error("Invalid GitHub release list");
    const values: readonly unknown[] = releases;
    for (const release of values) {
      if (
        typeof release !== "object" ||
        release === null ||
        !("tag_name" in release) ||
        typeof release.tag_name !== "string" ||
        !/^center-[0-9a-f]{40}$/.test(release.tag_name)
      )
        continue;
      if (
        !("draft" in release) ||
        release.draft !== false ||
        !("prerelease" in release) ||
        release.prerelease !== false
      )
        continue;
      if (!("id" in release) || typeof release.id !== "number")
        throw new Error("Missing published release ID");
      const bytes = await readSignedDescriptor({
        client,
        config,
        releaseId: release.id,
      });
      if (bytes === undefined)
        throw new Error("Published center release is incomplete");
      descriptors.push(parseDeliveryDescriptor(bytes));
    }
    if (releases.length < 100) return descriptors;
  }
  throw new Error("Published channel pagination limit exceeded");
};
