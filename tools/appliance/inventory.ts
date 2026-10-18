import { lstat, readdir } from "node:fs/promises";
import { join } from "node:path";
import { artifact } from "../release/manifest";
import type { Artifact } from "./types";
export const requireRelativeFile = async (
  directory: string,
  name: string,
): Promise<string> => {
  const parts = name.split("/");
  if (
    !/^[A-Za-z0-9_.+/-]+$/.test(name) ||
    parts.some((part) => !part || part === "." || part === "..")
  )
    throw new Error("Unsafe appliance artifact path");
  const root = await lstat(directory);
  if (!root.isDirectory() || root.isSymbolicLink())
    throw new Error("Unsafe artifact directory");
  for (let i = 1; i <= parts.length; i++) {
    const value = await lstat(join(directory, ...parts.slice(0, i)));
    if (
      value.isSymbolicLink() ||
      (i === parts.length ? !value.isFile() : !value.isDirectory())
    )
      throw new Error(
        "Appliance artifacts must be regular files without symlinks",
      );
  }
  return join(directory, name);
};
export const inventoryFiles = async ({
  directory,
  names,
}: Readonly<{
  directory: string;
  names: readonly string[];
}>): Promise<readonly Artifact[]> => {
  if (names.length === 0 || new Set(names).size !== names.length)
    throw new Error("Empty or duplicate appliance inventory");
  return Promise.all(
    [...names]
      .sort()
      .map(async (name) =>
        artifact({ path: await requireRelativeFile(directory, name), name }),
      ),
  );
};
export const verifyInventory = async ({
  directory,
  files,
}: Readonly<{
  directory: string;
  files: readonly Artifact[];
}>): Promise<void> => {
  const actual = await inventoryFiles({
    directory,
    names: files.map((file) => file.name),
  });
  const expected = new Map(files.map((file) => [file.name, file]));
  for (const file of actual) {
    const prior = expected.get(file.name);
    if (!prior || prior.sha256 !== file.sha256 || prior.size !== file.size)
      throw new Error(`Appliance artifact changed: ${file.name}`);
  }
};
export const listFiles = async (
  directory: string,
): Promise<readonly string[]> => {
  const visit = async (relative: string): Promise<string[]> => {
    const entries = await readdir(join(directory, relative), {
      withFileTypes: true,
    });
    const found: string[] = [];
    for (const entry of entries) {
      const name = relative ? `${relative}/${entry.name}` : entry.name;
      if (entry.isSymbolicLink())
        throw new Error("Unexpected symlink in appliance payload");
      if (entry.isDirectory()) found.push(...(await visit(name)));
      else if (entry.isFile()) found.push(name);
      else throw new Error("Unexpected special file in appliance payload");
    }
    return found;
  };
  return (await visit("")).sort();
};

export const checksums = (files: readonly Artifact[]): string =>
  files.map((file) => `${file.sha256}  ${file.name}\n`).join("");
