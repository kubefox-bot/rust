import { chmod } from "node:fs/promises";
import { join } from "node:path";
import { requireRelativeFile } from "../inventory";
import { run } from "../../lib/process";
export const normalizePayload = async ({
  payload,
  names,
}: Readonly<{
  payload: string;
  names: readonly string[];
}>): Promise<readonly string[]> => {
  const directories = new Set<string>(["."]);
  for (const name of names) {
    const file = await requireRelativeFile(payload, name);
    await chmod(file, 0o644);
    const parts = name.split("/");
    for (let length = 1; length < parts.length; length++)
      directories.add(parts.slice(0, length).join("/"));
  }
  for (const directory of directories)
    await chmod(join(payload, directory), 0o755);
  return [...directories].sort();
};
export const createPayloadArchive = async ({
  payload,
  names,
  tar,
  scratch,
}: Readonly<{
  payload: string;
  names: readonly string[];
  tar: string;
  scratch: string;
}>): Promise<void> => {
  const directories = await normalizePayload({ payload, names });
  await run({
    argv: [
      "tar",
      "--no-recursion",
      "--owner=0",
      "--group=0",
      "--numeric-owner",
      "-cf",
      tar,
      "-C",
      payload,
      ...directories,
      ...names,
    ],
    cwd: scratch,
  });
};
