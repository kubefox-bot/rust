import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { downloadVerified } from "../../lib/download";
test("bad or oversized downloads preserve previously verified bytes", async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-download-"));
  const path = join(root, "artifact");
  try {
    await Bun.write(path, "good");
    const request = {
      url: "https://example.test/artifact",
      sha256: Bun.SHA256.hash("good", "hex"),
      path,
      maxBytes: 5,
      timeoutMs: 1000,
    };
    await expect(
      downloadVerified({
        ...request,
        fetcher: async () => new Response("evil"),
      }),
    ).rejects.toThrow("mismatch");
    await expect(
      downloadVerified({
        ...request,
        fetcher: async () => new Response("oversized"),
      }),
    ).rejects.toThrow("limit");
    expect(await Bun.file(path).text()).toBe("good");
    await downloadVerified({
      ...request,
      fetcher: async () => new Response("good"),
    });
    expect(await Bun.file(path).text()).toBe("good");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
