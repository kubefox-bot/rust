import { resolve } from "node:path";
import { SOURCE_SHA_PATTERN } from "./conts";
import type { BuildOptions } from "./types";
export const readBuildOptions = (
  environment: Readonly<Record<string, string | undefined>> = Bun.env,
): BuildOptions => {
  const sourceSha = environment.HMAIL_DELIVERY_SOURCE_SHA;
  if (!sourceSha || !SOURCE_SHA_PATTERN.test(sourceSha))
    throw new Error("HMAIL_DELIVERY_SOURCE_SHA must be the full committed SHA");
  const root = resolve(import.meta.dir, "../..");
  const directory = (value: string | undefined, fallback: string): string => {
    const path = value ?? fallback;
    if (!path || /[\u0000-\u001f]/.test(path))
      throw new Error("Invalid appliance directory");
    return resolve(root, path);
  };
  return {
    root,
    sourceSha,
    releaseDirectory: directory(
      environment.HMAIL_DELIVERY_DIRECTORY,
      `target/releases/git-${sourceSha}`,
    ),
    output: directory(
      environment.HMAIL_APPLIANCE_OUTPUT,
      `target/appliance/git-${sourceSha}`,
    ),
    cache: directory(
      environment.HMAIL_APPLIANCE_CACHE,
      "target/appliance-cache",
    ),
  };
};
