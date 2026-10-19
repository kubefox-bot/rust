import { lstat } from "node:fs/promises";
import { join, relative } from "node:path";
import {
  EXCLUDED_SOURCE_PARTS,
  REVISION_PATTERN,
  SOURCE_INPUTS,
} from "./conts";
import { run } from "../lib/process";
import type { SourceIdentity } from "./types";

const included = (path: string) =>
  !path
    .split(/[\\/]/)
    .some(
      (part) =>
        EXCLUDED_SOURCE_PARTS.has(part) ||
        part.startsWith(".env") ||
        part.endsWith(".env") ||
        part.endsWith(".log"),
    );
const sourceFiles = async (root: string): Promise<readonly string[]> => {
  const files: string[] = [];
  for (const input of SOURCE_INPUTS) {
    const path = join(root, input);
    const metadata = await lstat(path).catch((error: unknown) => {
      if (error instanceof Error && "code" in error && error.code === "ENOENT")
        return undefined;
      throw error;
    });
    if (!metadata) continue;
    if (metadata.isSymbolicLink())
      throw new Error("Release source must not contain symbolic links");
    if (metadata.isFile()) {
      files.push(input);
      continue;
    }
    for await (const entry of new Bun.Glob("**/*").scan({
      cwd: path,
      dot: true,
      onlyFiles: true,
      followSymlinks: false,
    })) {
      const name = relative(root, join(path, entry));
      if (included(name)) files.push(name);
    }
  }
  return [...new Set(files)].sort();
};
export const sourceFingerprint = async (root: string): Promise<string> => {
  const hash = new Bun.CryptoHasher("sha256");
  for (const name of await sourceFiles(root)) {
    const path = join(root, name);
    const metadata = await lstat(path);
    if (!metadata.isFile())
      throw new Error("Release source must contain regular files only");
    hash.update(`${name}\0${metadata.size}\0`);
    for await (const bytes of Bun.file(path).stream()) hash.update(bytes);
  }
  return hash.digest("hex");
};
export const sourceIdentity = async (root: string): Promise<SourceIdentity> => {
  const revision = (
    await run({ argv: ["git", "rev-parse", "--short=12", "HEAD"], cwd: root })
  ).trim();
  if (!REVISION_PATTERN.test(revision))
    throw new Error("Cannot resolve the source revision");
  const changed = await run({
    argv: ["git", "diff", "--name-only", "-z", "HEAD", "--", ...SOURCE_INPUTS],
    cwd: root,
  });
  const untracked = await run({
    argv: [
      "git",
      "ls-files",
      "--others",
      "--exclude-standard",
      "-z",
      "--",
      ...SOURCE_INPUTS,
    ],
    cwd: root,
  });
  const dirty = `${changed}${untracked}`
    .split("\0")
    .filter(Boolean)
    .some(included);
  const fingerprint = await sourceFingerprint(root);
  return {
    revision: dirty
      ? `${revision}-dirty.${fingerprint.slice(0, 12)}`
      : revision,
    fingerprint,
    dirty,
  };
};
