import { PACKAGE_DOWNLOAD_LIMIT, PACKAGE_DOWNLOAD_TIMEOUT_MS } from "./conts";
export const downloadPinned = async ({
  url,
  sha256,
  path,
}: Readonly<{ url: string; sha256: string; path: string }>): Promise<void> => {
  const response = await fetch(url, {
    signal: AbortSignal.timeout(PACKAGE_DOWNLOAD_TIMEOUT_MS),
    redirect: "follow",
  });
  if (
    !response.ok ||
    !response.body ||
    (response.headers.has("content-length") &&
      Number(response.headers.get("content-length")) > PACKAGE_DOWNLOAD_LIMIT)
  )
    throw new Error("Pinned tool download failed or exceeded limit");
  const reader = response.body.getReader();
  const hash = new Bun.CryptoHasher("sha256");
  const chunks: Uint8Array[] = [];
  let size = 0;
  try {
    for (;;) {
      const item = await reader.read();
      if (item.done) break;
      size += item.value.byteLength;
      if (size > PACKAGE_DOWNLOAD_LIMIT)
        throw new Error("Pinned tool download exceeded limit");
      hash.update(item.value);
      chunks.push(item.value);
    }
  } finally {
    await reader.cancel();
    reader.releaseLock();
  }
  if (size === 0 || hash.digest("hex") !== sha256)
    throw new Error("Pinned tool SHA-256 mismatch");
  await Bun.write(path, Buffer.concat(chunks));
};
