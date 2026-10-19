import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { terminateProcessGroup } from "../../lib/process-group";
import { deadline } from "./deadline";
import type { OwnedFixtureProcess } from "./types";
export const ownFixtureProcess = async (
  input: Readonly<{
    launch: (
      temporaryRoot: string,
    ) => Bun.Subprocess<"pipe", "pipe", "inherit">;
    exitGraceMs: number;
    termGraceMs: number;
    killGraceMs: number;
  }>,
): Promise<OwnedFixtureProcess> => {
  const temporaryRoot = await mkdtemp(
    join(tmpdir(), "hmail-mvp-synthetic-resource-owner-"),
  );
  const launch = () => {
    try {
      return input.launch(temporaryRoot);
    } catch (error) {
      throw new Error("Could not launch the owned resource fixture", {
        cause: error,
      });
    }
  };
  const child = await Promise.resolve()
    .then(launch)
    .catch(async (error: unknown) => {
      await rm(temporaryRoot, { recursive: true });
      throw error;
    });
  const cancellation = new AbortController();
  const releases: Array<() => Promise<void>> = [];
  let stopping: Promise<void> | undefined;
  const stop = (): Promise<void> => {
    if (stopping) return stopping;
    stopping = (async () => {
      process.removeListener("SIGINT", interrupted);
      process.removeListener("SIGTERM", terminated);
      const errors: unknown[] = [];
      for (const release of releases.reverse()) {
        try {
          await deadline(release(), input.exitGraceMs);
        } catch (error) {
          errors.push(error);
        }
      }
      try {
        await deadline(
          Promise.resolve(child.stdin.end()).then(() => undefined),
          input.exitGraceMs,
        );
      } catch (error) {
        if (child.exitCode === null) errors.push(error);
      }
      // The leader may exit while the actual center descendant is still alive.
      // Always terminate/check the owned group, even after leader completion.
      try {
        await deadline(child.exited, input.exitGraceMs).catch(() => undefined);
        const group = await terminateProcessGroup({
          pid: child.pid,
          termGraceMs: input.termGraceMs,
          killGraceMs: input.killGraceMs,
        });
        await deadline(child.exited, input.killGraceMs);
        if (!group.gone)
          throw new Error(
            `Resource fixture process group remains alive; private files retained at ${temporaryRoot}`,
          );
        await rm(temporaryRoot, { recursive: true });
      } catch (error) {
        errors.push(error);
      }
      if (errors.length > 0)
        throw new AggregateError(errors, "Resource fixture cleanup failed");
    })();
    return stopping;
  };
  const cancel = (signal: "SIGINT" | "SIGTERM") => {
    cancellation.abort(
      new Error(`Resource measurement cancelled by ${signal}`),
    );
    process.exitCode = signal === "SIGINT" ? 130 : 143;
    stop().catch((error: unknown) => {
      console.error(error);
      process.exitCode = 1;
    });
  };
  const interrupted = () => cancel("SIGINT");
  const terminated = () => cancel("SIGTERM");
  process.once("SIGINT", interrupted);
  process.once("SIGTERM", terminated);
  return {
    child,
    temporaryRoot,
    signal: cancellation.signal,
    stop,
    retain: async (release) => {
      if (stopping) {
        await release();
        throw new Error("Resource fixture was already closed");
      }
      releases.push(release);
    },
  };
};
