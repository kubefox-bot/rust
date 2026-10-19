import { mkdtemp, rename, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
// Keeps a previously verified artifact intact until all new bytes pass SHA-256.
export const downloadVerified = async ({
  url,
  sha256,
  path,
  maxBytes,
  timeoutMs,
  fetcher = fetch,
}: Readonly<{
  url: string;
  sha256: string;
  path: string;
  maxBytes: number;
  timeoutMs: number;
  fetcher?: (url: URL, init: RequestInit) => Promise<Response>;
}>): Promise<void> => {
  const address = new URL(url);
  if (
    address.protocol !== "https:" ||
    address.username ||
    address.password ||
    !/^[0-9a-f]{64}$/.test(sha256) ||
    !Number.isSafeInteger(maxBytes) ||
    maxBytes < 1
  )
    throw new Error("Invalid verified download request");
  const scratch = await mkdtemp(join(dirname(path), ".download-"));
  try {
    const response = await fetcher(address, {
      signal: AbortSignal.timeout(timeoutMs),
      redirect: "follow",
    });
    if (
      !response.ok ||
      !response.body ||
      (response.url && new URL(response.url).protocol !== "https:") ||
      (response.headers.has("content-length") &&
        Number(response.headers.get("content-length")) > maxBytes)
    ) {
      await response.body?.cancel();
      throw new Error("Verified download failed or exceeded limit");
    }
    const hash = new Bun.CryptoHasher("sha256");
    let size = 0;
    const checked = response.body.pipeThrough(
      new TransformStream<Uint8Array, Uint8Array>({
        transform(chunk, controller) {
          size += chunk.byteLength;
          if (size > maxBytes)
            throw new Error("Verified download exceeded limit");
          hash.update(chunk);
          controller.enqueue(chunk);
        },
      }),
    );
    const temporary = join(scratch, "download");
    await Bun.write(temporary, new Response(checked));
    if (size === 0 || hash.digest("hex") !== sha256)
      throw new Error("Verified download SHA-256 mismatch");
    await rename(temporary, path);
  } catch (error) {
    try {
      await rm(scratch, { recursive: true, force: true });
    } catch (cleanup) {
      throw new AggregateError([error, cleanup], "Download and cleanup failed");
    }
    throw error;
  }
  await rm(scratch, { recursive: true, force: true });
};
