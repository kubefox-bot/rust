import type { PromotionInput } from "./types";
export const promoteRelease = async ({
  release,
  descriptor,
  ports,
}: PromotionInput): Promise<"promoted" | "superseded"> => {
  if (
    release.sourceSha !== descriptor.source_sha ||
    release.sequence !== descriptor.sequence ||
    release.imageDigest !== descriptor.image.digest
  )
    throw new Error("Promotion identity mismatch");
  if ((await ports.currentHead()) !== release.sourceSha) return "superseded";
  for (const current of await ports.publishedDescriptors()) {
    if (current.sequence > descriptor.sequence) return "superseded";
    if (
      current.sequence === descriptor.sequence &&
      (current.image.digest !== descriptor.image.digest ||
        current.source_sha !== descriptor.source_sha ||
        current.release_manifest_sha256 !== descriptor.release_manifest_sha256)
    )
      throw new Error("Release sequence conflict");
  }
  if ((await ports.currentHead()) !== release.sourceSha) return "superseded";
  await ports.publishDraft(release.releaseId);
  return "promoted";
};
