import { artifact } from "../manifest";
import { releaseAssets } from "./release";
import { listAssets } from "./assets";
import { restoreHostSignature } from "./host-package";
import { readSignedDescriptor } from "./channel";
import type { GithubClient } from "./github";
import type { PublicationConfig } from "./types";

export const createDraft = async ({
  client,
  config,
}: Readonly<{ client: GithubClient; config: PublicationConfig }>): Promise<
  Readonly<{ id: number; published: boolean; descriptor?: string }>
> => {
  const tag = `center-${config.sourceSha}`;
  const response = await client.request({ path: `/releases/tags/${tag}` });
  const value: unknown =
    response.status === 404
      ? await client.json({
          path: "/releases",
          method: "POST",
          body: {
            tag_name: tag,
            target_commitish: config.sourceSha,
            name: `Astra git-${config.sourceSha}`,
            draft: true,
            prerelease: false,
            body: "Verified same-SHA center image, agent package and deployment metadata.",
          },
        })
      : JSON.parse(await client.text(response));
  if (
    typeof value !== "object" ||
    value === null ||
    !("id" in value) ||
    typeof value.id !== "number" ||
    !("draft" in value) ||
    typeof value.draft !== "boolean" ||
    !("target_commitish" in value) ||
    value.target_commitish !== config.sourceSha
  )
    throw new Error("Existing release identity conflict");
  if (!value.draft) {
    await restoreHostSignature({ client, config, releaseId: value.id });
    return {
      id: value.id,
      published: true,
      descriptor: await readSignedDescriptor({
        client,
        config,
        releaseId: value.id,
        retainDirectory: config.directory,
      }),
    };
  }
  const assets = await listAssets(client, value.id);
  const descriptor = assets.find((asset) => asset.name === "delivery.json");
  const bytes = descriptor
    ? await client.text(
        await client.request({
          path: `/releases/assets/${descriptor.id}`,
          accept: "application/octet-stream",
        }),
      )
    : undefined;
  return { id: value.id, published: false, descriptor: bytes };
};
export const uploadAssets = async ({
  client,
  releaseId,
  files,
}: Readonly<{
  client: GithubClient;
  releaseId: number;
  files: ReadonlyArray<Readonly<{ name: string; path: string }>>;
}>): Promise<void> => {
  const existing = await listAssets(client, releaseId);
  for (const file of files) {
    const actual = await artifact(file);
    const previous = existing.find((asset) => asset.name === file.name);
    if (
      previous?.digest === `sha256:${actual.sha256}` &&
      previous.size === actual.size
    )
      continue;
    if (previous) {
      const response = await client.request({
        path: `/releases/assets/${previous.id}`,
        method: "DELETE",
      });
      if (response.status !== 204)
        throw new Error("Cannot replace draft release asset");
    }
    await client.upload({ releaseId, ...file });
  }
};
export const verifyAssets = async ({
  client,
  config,
  releaseId,
  names,
}: Readonly<{
  client: GithubClient;
  config: PublicationConfig;
  releaseId: number;
  names: readonly string[];
}>): Promise<void> => {
  const actual = await listAssets(client, releaseId);
  const paths = releaseAssets({
    directory: config.directory,
    build: `git-${config.sourceSha}`,
  });
  for (const name of names) {
    const asset = actual.find((asset) => asset.name === name);
    const file = paths.find((file) => file.name === name);
    if (!asset || !file) throw new Error("Release is missing a required asset");
    const local = await artifact(file);
    if (asset.size !== local.size || asset.digest !== `sha256:${local.sha256}`)
      throw new Error("Uploaded asset digest or size mismatch");
  }
};
