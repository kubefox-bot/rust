import { mkdir, open } from "node:fs/promises";
import { basename, join } from "node:path";
import { run } from "../lib/process";
import { artifact } from "../release/manifest";
import { downloadVerified } from "../lib/download";
import { DOWNLOAD_TIMEOUT_MS, MAX_DOWNLOAD_BYTES } from "./conts";
const cachedZip = async ({
  url,
  sha256,
  cache,
}: Readonly<{
  url: string;
  sha256: string;
  cache: string;
}>): Promise<string> => {
  await mkdir(cache, { recursive: true, mode: 0o700 });
  const path = join(cache, `${sha256}.zip`);
  if (await Bun.file(path).exists()) {
    if ((await artifact({ path, name: "upstream.zip" })).sha256 !== sha256)
      throw new Error("Cached upstream bytes changed");
    return path;
  }
  await downloadVerified({
    url,
    sha256,
    path,
    maxBytes: MAX_DOWNLOAD_BYTES,
    timeoutMs: DOWNLOAD_TIMEOUT_MS,
  });
  return path;
};
export const unpackPinnedZip = async ({
  url,
  sha256,
  cache,
  output,
  member,
}: Readonly<{
  url: string;
  sha256: string;
  cache: string;
  output: string;
  member: string;
}>): Promise<void> => {
  if (basename(member) !== member || !/^[A-Za-z0-9._-]+$/.test(member))
    throw new Error("Unsafe ZIP member");
  const zip = await cachedZip({ url, sha256, cache });
  const names = (await run({ argv: ["unzip", "-Z1", zip], cwd: cache }))
    .trim()
    .split("\n");
  if (names.length !== 1 || names[0] !== member)
    throw new Error("Unexpected upstream archive members");
  const target = await open(output, "wx", 0o600);
  try {
    const child = Bun.spawn(["unzip", "-p", zip, member], {
      stdout: target.fd,
      stderr: "pipe",
    });
    const [code, error] = await Promise.all([
      child.exited,
      new Response(child.stderr).text(),
    ]);
    if (code !== 0) throw new Error(`Upstream ZIP extraction failed: ${error}`);
    await target.sync();
  } finally {
    await target.close();
  }
};
