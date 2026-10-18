import type { GithubClient } from "./github";
export type ReleaseAsset = Readonly<{
  id: number;
  name: string;
  size: number;
  digest?: string;
}>;
export const readAssets = (value: unknown): readonly ReleaseAsset[] => {
  if (!Array.isArray(value)) throw new Error("Invalid GitHub release assets");
  return value.map((asset: unknown) => {
    if (
      typeof asset !== "object" ||
      asset === null ||
      !("id" in asset) ||
      typeof asset.id !== "number" ||
      !Number.isSafeInteger(asset.id) ||
      !("name" in asset) ||
      typeof asset.name !== "string" ||
      !("size" in asset) ||
      typeof asset.size !== "number" ||
      !("state" in asset) ||
      asset.state !== "uploaded"
    )
      throw new Error("Incomplete GitHub release asset");
    return {
      id: asset.id,
      name: asset.name,
      size: asset.size,
      digest:
        "digest" in asset && typeof asset.digest === "string"
          ? asset.digest
          : undefined,
    };
  });
};
export const listAssets = async (
  client: GithubClient,
  id: number,
): Promise<readonly ReleaseAsset[]> => {
  const values: ReleaseAsset[] = [];
  for (let page = 1; page <= 20; page++) {
    const assets = readAssets(
      await client.json({
        path: `/releases/${id}/assets?per_page=100&page=${page}`,
      }),
    );
    values.push(...assets);
    if (assets.length < 100) return values;
  }
  throw new Error("Too many release assets");
};
