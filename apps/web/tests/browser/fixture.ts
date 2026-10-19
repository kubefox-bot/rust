import { chromium } from "playwright";
import {
  acknowledged,
  readyMessage,
  receiptMessage,
  statsMessage,
} from "./fixture-protocol";
import { lineReader } from "./line-reader";
import type { BrowserContracts } from "../../src/api";
import {
  FIXTURE_BINARY,
  FIXTURE_SHUTDOWN_MS,
  FIXTURE_STARTUP_MS,
  ROOT,
  STATIC_DIRECTORY,
} from "./config";
const deadline = async <Value>(
  operation: Promise<Value>,
  milliseconds: number,
): Promise<Value> => {
  let timer: ReturnType<typeof setTimeout> | undefined = undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error("Fixture deadline exceeded")),
          milliseconds,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
};
export const startFixture = async (
  options: Readonly<{ historyRows?: number }> = {},
) => {
  const child = Bun.spawn(
    [FIXTURE_BINARY, STATIC_DIRECTORY, String(options.historyRows ?? 0)],
    {
      cwd: ROOT,
      stdin: "pipe",
      stdout: "pipe",
      stderr: "inherit",
    },
  );
  const output = lineReader(child.stdout);
  const stop = async () => {
    await child.stdin.end();
    await deadline(child.exited, FIXTURE_SHUTDOWN_MS).catch(async (error) => {
      child.kill("SIGKILL");
      await child.exited;
      throw error;
    });
    await output.close();
  };
  try {
    const ready = readyMessage(
      await deadline(output.line(), FIXTURE_STARTUP_MS),
    );
    const browser = await chromium.launch({
      headless: true,
      args: [`--ignore-certificate-errors-spki-list=${ready.spki}`],
    });
    try {
      const context = await browser.newContext();
      let busy = false;
      const command = async (
        input: Readonly<Record<string, unknown>>,
      ): Promise<unknown> => {
        if (busy)
          throw new Error("Concurrent fixture commands are not supported");
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
        finishLatest: async (serverId: string) =>
          acknowledged(
            await command({ kind: "finish_latest", server_id: serverId }),
          ),
        phase: async (serverId: string, phase: string) =>
          acknowledged(
            await command({ kind: "phase", server_id: serverId, phase }),
          ),
        loseResponse: async (serverId: string, enabled: boolean) =>
          acknowledged(
            await command({
              kind: "lose_response",
              server_id: serverId,
              enabled,
            }),
          ),
        hideLookup: async (serverId: string, enabled: boolean) =>
          acknowledged(
            await command({
              kind: "hide_lookup",
              server_id: serverId,
              enabled,
            }),
          ),
        stats: async () => statsMessage(await command({ kind: "stats" })),
        receipt: async (serverId: string) =>
          receiptMessage(
            await command({ kind: "receipt", server_id: serverId }),
          ),
        mailState: async (
          serverId: string,
          state: BrowserContracts["profile_validation"]["mail_state"],
        ) =>
          acknowledged(
            await command({ kind: "mail_state", server_id: serverId, state }),
          ),
        close: async () => {
          try {
            await browser.close();
          } finally {
            await stop();
          }
        },
      };
    } catch (error) {
      await browser.close();
      throw error;
    }
  } catch (error) {
    await stop();
    throw error;
  }
};
