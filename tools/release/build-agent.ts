import { copyFile, mkdir } from "node:fs/promises";
import { join } from "node:path";
import { AGENT_EXECUTABLE, PRIVATE_DIRECTORY_MODE } from "./conts";
import { validateAgentPackage } from "./manifest";
import { run } from "../lib/process";
import type { ReleaseConfig } from "./types";

export const buildAgentPackage = async (
  config: ReleaseConfig,
): Promise<string> => {
  await run({
    argv: ["sh", "tools/build-exe.sh", "--component", "agent"],
    cwd: config.root,
    env: config.environment,
    inherit: true,
  });
  const source = join(config.root, "dist/agent/windows-x64");
  await validateAgentPackage({ directory: source, build: config.build });
  const destination = join(config.directory, "agent-packages", config.build);
  await mkdir(destination, { recursive: true, mode: PRIVATE_DIRECTORY_MODE });
  for (const name of [
    AGENT_EXECUTABLE,
    `${AGENT_EXECUTABLE}.sha256`,
    "manifest.json",
  ])
    await copyFile(join(source, name), join(destination, name));
  return destination;
};
