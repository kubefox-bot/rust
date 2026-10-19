import { chromium } from "playwright";
import { join } from "node:path";
import {
  readyMessage,
  acknowledged,
} from "../../../apps/web/tests/browser/fixture-protocol";
import { lineReader } from "../../../apps/web/tests/browser/line-reader";
import { validateFixtureTarget } from "../fixture-target";
import {
  FIXTURE_STARTUP_MS,
  FIXTURE_SHUTDOWN_MS,
  FIXTURE_TERM_GRACE_MS,
  FIXTURE_KILL_GRACE_MS,
  NATIVE_SERVER_CONNECT_TIMEOUT_MS,
  NATIVE_SERVER_START_RETRY_MS,
} from "./conts";
import { run } from "../../lib/process";
import type { MeasurementConfig } from "./types";
import { deadline } from "./deadline";
import { ownFixtureProcess } from "./fixture-process";
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
export const resourceMetadata = (value: unknown) => {
  if (
    !record(value) ||
    !Number.isSafeInteger(value.center_pid) ||
    typeof value.center_pid !== "number" ||
    value.center_pid <= 0 ||
    typeof value.root !== "string" ||
    typeof value.fixture_id !== "string"
  )
    throw new Error("Invalid resource fixture process metadata");
  return {
    ...readyMessage(value),
    centerPid: value.center_pid,
    fixtureId: value.fixture_id,
    root: value.root,
  };
};
export const startResourceFixture = async (config: MeasurementConfig) => {
  const owned = await ownFixtureProcess({
    launch: (temporaryRoot) =>
      Bun.spawn(
        [
          config.fixtureBinary,
          config.centerBinary,
          config.staticDirectory,
          temporaryRoot,
        ],
        {
          cwd: config.root,
          detached: true,
          stdin: "pipe",
          stdout: "pipe",
          stderr: "inherit",
        },
      ),
    exitGraceMs: FIXTURE_SHUTDOWN_MS,
    termGraceMs: FIXTURE_TERM_GRACE_MS,
    killGraceMs: FIXTURE_KILL_GRACE_MS,
  });
  const child = owned.child;
  const output = lineReader(child.stdout);
  await owned.retain(() => output.close());
  try {
    const ready = resourceMetadata(
      await deadline(output.line(), FIXTURE_STARTUP_MS),
    );
    owned.signal.throwIfAborted();
    await validateFixtureTarget({
      fixture_id: ready.fixtureId,
      root: ready.root,
      data: join(ready.root, "synthetic-mail"),
      service: "hmail-mvp-fixture-resource",
    });
    const parentPid = await run({
      argv: ["ps", "-p", String(ready.centerPid), "-o", "ppid="],
      cwd: config.root,
    });
    if (Number(parentPid) !== child.pid)
      throw new Error("Measured process is not owned by the resource fixture");
    const browser = await chromium.launch({
      headless: true,
      args: [`--ignore-certificate-errors-spki-list=${ready.spki}`],
    });
    await owned.retain(() => browser.close());
    owned.signal.throwIfAborted();
    try {
      const context = await browser.newContext();
      // Only the newly generated fixture CA SPKI is trusted by this browser process.
      const readyPage = await context.newPage();
      const health = await deadline(
        (async (): Promise<Readonly<{ buildVersion: string }>> => {
          while (true) {
            try {
              const response = await readyPage.goto(`${ready.origin}/healthz`, {
                timeout: NATIVE_SERVER_CONNECT_TIMEOUT_MS,
              });
              if (response?.ok()) {
                const value: unknown = await response.json();
                if (
                  !record(value) ||
                  value.status !== "ok" ||
                  typeof value.build_version !== "string"
                )
                  throw new Error("Invalid center health response");
                return { buildVersion: value.build_version };
              }
            } catch (error) {
              if (
                !(error instanceof Error) ||
                !error.message.includes("ERR_CONNECTION_REFUSED")
              )
                throw error;
            }
            await Bun.sleep(NATIVE_SERVER_START_RETRY_MS);
          }
        })(),
        FIXTURE_STARTUP_MS,
      );
      await readyPage.close();
      let busy = false;
      const command = async (
        input: Readonly<Record<string, unknown>>,
      ): Promise<unknown> => {
        if (busy)
          throw new Error(
            "Concurrent resource fixture commands are not supported",
          );
        busy = true;
        try {
          child.stdin.write(`${JSON.stringify(input)}\n`);
          await child.stdin.flush();
          return await deadline(output.line(), FIXTURE_STARTUP_MS);
        } finally {
          busy = false;
        }
      };
      return {
        ...ready,
        context,
        browserVersion: browser.version(),
        centerBuildVersion: health.buildVersion,
        finish: async (serverId: string) =>
          acknowledged(
            await command({ kind: "finish_latest", server_id: serverId }),
          ),
        phase: async (serverId: string, phase: string) =>
          acknowledged(
            await command({ kind: "phase", server_id: serverId, phase }),
          ),
        stopMail: async (serverId: string) =>
          acknowledged(
            await command({ kind: "mail_stopped", server_id: serverId }),
          ),
        stats: async () => command({ kind: "stats" }),
        close: owned.stop,
      };
    } catch (error) {
      throw error;
    }
  } catch (error) {
    try {
      await owned.stop();
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        "Resource fixture startup and cleanup failed",
      );
    }
    throw error;
  }
};
