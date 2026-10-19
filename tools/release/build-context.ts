import { cp, lstat, mkdir, copyFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import {
  CONTAINER_INPUTS,
  EXCLUDED_SOURCE_PARTS,
  PRIVATE_DIRECTORY_MODE,
} from "./conts";
import { pinContainerfile } from "./containerfile";
import type { BaseImages } from "./types";

export const stageBuildContext = async ({
  root,
  directory,
  bases,
}: Readonly<{
  root: string;
  directory: string;
  bases: BaseImages;
}>): Promise<string> => {
  const destination = join(directory, "build-context");
  await mkdir(destination, { mode: PRIVATE_DIRECTORY_MODE });
  for (const input of CONTAINER_INPUTS) {
    await mkdir(dirname(join(destination, input)), {
      recursive: true,
      mode: PRIVATE_DIRECTORY_MODE,
    });
    await cp(join(root, input), join(destination, input), {
      recursive: true,
      filter: async (path) => {
        const parts = relative(root, path).split(/[\\/]/);
        if (
          parts.some(
            (part) =>
              EXCLUDED_SOURCE_PARTS.has(part) ||
              part.startsWith(".env") ||
              part.endsWith(".env") ||
              part.endsWith(".log"),
          )
        )
          return false;
        if ((await lstat(path)).isSymbolicLink())
          throw new Error(
            "Container build context must not contain symbolic links",
          );
        return true;
      },
    });
  }
  await copyFile(
    join(root, ".containerignore"),
    join(destination, ".containerignore"),
  );
  await copyFile(
    join(root, ".containerignore"),
    join(destination, ".dockerignore"),
  );
  const containerfile = join(destination, "deploy/Containerfile");
  await Bun.write(
    containerfile,
    pinContainerfile({
      source: await Bun.file(containerfile).text(),
      bases,
    }),
  );
  return destination;
};
