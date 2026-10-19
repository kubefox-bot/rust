import { executeProcess } from "./execute";
import type { CapturedProcessOptions } from "./types";

export const readProcessOutput = async (options: CapturedProcessOptions) => {
  const stdout: Uint8Array[] = [];
  const stderr: Uint8Array[] = [];
  const outcome = await executeProcess({
    ...options,
    output: {
      stdout: (chunk) => {
        stdout.push(chunk);
        return chunk.byteLength;
      },
      stderr: (chunk) => {
        stderr.push(chunk);
        return chunk.byteLength;
      },
    },
  });
  if (outcome.kind === "stopped") throw new Error(outcome.reason);
  const error = Buffer.concat(stderr).toString("utf8");
  if (outcome.code !== 0)
    throw new Error(
      `Process exited with code ${outcome.code}: ${error.trim()}`,
    );
  return { stdout: Buffer.concat(stdout).toString("utf8"), stderr: error };
};
