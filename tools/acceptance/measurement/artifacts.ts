import { realpath, lstat, readdir } from "node:fs/promises";
import { join, resolve } from "node:path";
export const nativeArtifact = async (
  input: Readonly<{ root: string; relative: string }>,
): Promise<string> => {
  if (!/^target\/release\/(?:examples\/)?[a-zA-Z0-9_-]+$/.test(input.relative))
    throw new Error("Artifact must use an exact repository release path");
  const root = await realpath(input.root);
  const expected = resolve(root, input.relative);
  if (
    (await realpath(expected)) !== expected ||
    !(await lstat(expected)).isFile()
  )
    throw new Error(
      "Native artifact must be a regular file at its exact repository path",
    );
  return expected;
};
export const staticArtifacts = async (
  root: string,
): Promise<ReadonlyArray<Readonly<{ path: string; bytes: number }>>> => {
  const entries = await readdir(root, { withFileTypes: true });
  const output: Array<Readonly<{ path: string; bytes: number }>> = [];
  for (const entry of entries) {
    const path = join(root, entry.name);
    if (entry.isSymbolicLink())
      throw new Error("Static artifact tree must not contain symlinks");
    if (entry.isDirectory()) output.push(...(await staticArtifacts(path)));
    else if (entry.isFile())
      output.push({ path, bytes: (await lstat(path)).size });
  }
  return output;
};
