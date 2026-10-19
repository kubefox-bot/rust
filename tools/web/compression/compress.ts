import { randomUUID } from "node:crypto";
import {
  lstat,
  readFile,
  readdir,
  rename,
  rm,
  writeFile,
} from "node:fs/promises";
import { extname, join } from "node:path";
import { brotliCompressSync, gzipSync } from "node:zlib";
import { BROTLI_OPTIONS, COMPRESSIBLE_EXTENSIONS, GZIP_OPTIONS } from "./conts";

const writeSidecar = async ({
  path,
  content,
}: Readonly<{ path: string; content: Uint8Array }>) => {
  const temporaryPath = `${path}.${randomUUID()}.tmp`;
  try {
    await writeFile(temporaryPath, content, { flag: "wx" });
    await rename(temporaryPath, path);
  } catch (error) {
    try {
      await rm(temporaryPath, { force: true });
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        "Sidecar creation and temporary-file cleanup failed",
      );
    }
    throw error;
  }
};

const compressDirectory = async (directory: string): Promise<void> => {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      await compressDirectory(path);
      continue;
    }
    if (!entry.isFile() || !COMPRESSIBLE_EXTENSIONS.has(extname(entry.name)))
      continue;
    const bytes = await readFile(path);
    await writeSidecar({
      path: `${path}.br`,
      content: brotliCompressSync(bytes, BROTLI_OPTIONS),
    });
    await writeSidecar({
      path: `${path}.gz`,
      content: gzipSync(bytes, GZIP_OPTIONS),
    });
  }
};

export const precompressAssets = async ({
  directory,
}: Readonly<{ directory: string }>): Promise<void> => {
  const assetsDirectory = join(directory, "_astro");
  if (!(await lstat(assetsDirectory)).isDirectory())
    throw new Error("Build assets must be a directory");
  await compressDirectory(assetsDirectory);
};
