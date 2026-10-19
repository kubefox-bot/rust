import { expect, test } from "bun:test";
import { chmod, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { verifyMysqlFixture } from "../mysql-fixture";
import { runAcceptance } from "../run";
import type { AcceptanceConfig } from "../types";

const posixTest = process.platform === "win32" ? test.skip : test;
const policy = {
  timeoutMs: 1000,
  termGraceMs: 50,
  killGraceMs: 500,
  drainMs: 100,
};
const bounded = async <T>(pending: Promise<T>): Promise<T> => {
  const deadline = Promise.withResolvers<never>();
  const timer = setTimeout(
    () =>
      deadline.reject(new Error("MySQL guard did not finish within its bound")),
    1800,
  );
  try {
    return await Promise.race([pending, deadline.promise]);
  } finally {
    clearTimeout(timer);
  }
};
const alive = (pid: number) => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ESRCH")
      return false;
    throw error;
  }
};
const waitForPid = async (path: string) => {
  for (let attempt = 0; attempt < 300; attempt += 1) {
    if (await Bun.file(path).exists())
      return Number(await Bun.file(path).text());
    await Bun.sleep(10);
  }
  throw new Error("Synthetic Docker guard did not start");
};
const fixture = async ({
  hanging = false,
  endpoint = "unix:///tmp/synthetic-docker.sock",
  project = "hmail-cleanup-tests",
  binding = "127.0.0.1:13306",
}: Readonly<{
  hanging?: boolean;
  endpoint?: string;
  project?: string;
  binding?: string;
}> = {}) => {
  const root = await mkdtemp(join(tmpdir(), "hmail-mysql-guard-"));
  const bin = join(root, "bin");
  const calls = join(root, "calls.jsonl");
  const pidFile = join(root, "docker.pid");
  await mkdir(bin);
  const record = `import { appendFile } from "node:fs/promises"; await appendFile(${JSON.stringify(calls)}, JSON.stringify({ command: process.argv[1].split("/").at(-1), argv: process.argv.slice(2) }) + ${JSON.stringify("\n")});`;
  const docker = hanging
    ? `process.on("SIGTERM", () => {}); await Bun.write(${JSON.stringify(pidFile)}, String(process.pid)); setInterval(() => {}, 1000);`
    : `const argv = process.argv.slice(2); console.error("harmless stderr diagnostic"); if (argv[0] === "context") console.log(${JSON.stringify(endpoint)}); else if (argv.includes('{{ index .Config.Labels "com.docker.compose.project" }}')) console.log(${JSON.stringify(project)}); else console.log(${JSON.stringify(binding)});`;
  for (const command of ["bun", "mise", "cargo", "docker"]) {
    const path = join(bin, command);
    await Bun.write(
      path,
      `#!${process.execPath}\n${record}\n${command === "docker" ? docker : ""}\n`,
    );
    await chmod(path, 0o700);
  }
  const config: AcceptanceConfig = {
    root,
    output: root,
    database: true,
    dockerContext: "colima",
    deliveryManifest: undefined,
    environment: { PATH: bin },
  };
  return {
    root,
    config,
    calls,
    pidFile,
    kill: async () => {
      if (!(await Bun.file(pidFile).exists())) return;
      const pid = Number(await Bun.file(pidFile).text());
      if (alive(pid)) process.kill(pid, "SIGKILL");
    },
    close: () => rm(root, { recursive: true, force: true }),
  };
};

posixTest(
  "cancelling a hanging MySQL guard writes FAIL and starts no SQL checks",
  async () => {
    const f = await fixture({ hanging: true });
    const controller = new AbortController();
    const priorExitCode = process.exitCode;
    const pending = runAcceptance(f.config, {
      signal: controller.signal,
      policy: { ...policy, timeoutMs: 10000 },
    });
    try {
      const pid = await waitForPid(f.pidFile);
      controller.abort();
      const report = await bounded(pending);
      expect(alive(pid)).toBe(false);
      expect(report.checks).toContainEqual({
        name: "mysql-fixture",
        verdict: "FAIL",
        reason: expect.stringContaining("cancelled"),
      });
      for (const name of ["test-db", "test-agent-db"])
        expect(report.checks).toContainEqual({
          name,
          verdict: "NOT RUN",
          reason: expect.any(String),
        });
      const commands = await Bun.file(f.calls).text();
      expect(commands).not.toContain('"test:db"');
      expect(commands).not.toContain('"test:agent-db"');
      expect(
        (await Bun.file(join(f.root, "report.json")).json()).checks,
      ).toEqual(report.checks);
    } finally {
      await f.kill();
      await Promise.allSettled([pending]);
      process.exitCode = priorExitCode ?? 0;
      await f.close();
    }
  },
);

posixTest(
  "a hanging Docker proof read has its own finite timeout",
  async () => {
    const f = await fixture({ hanging: true });
    const pending = verifyMysqlFixture(f.config, { policy });
    const rejected = expect(bounded(pending)).rejects.toThrow("timed out");
    try {
      const pid = await waitForPid(f.pidFile);
      await rejected;
      expect(alive(pid)).toBe(false);
    } finally {
      await f.kill();
      await Promise.allSettled([pending]);
      await f.close();
    }
  },
);

posixTest(
  "Docker proof reads keep stderr separate and preserve every fixture guard",
  async () => {
    const good = await fixture();
    try {
      await expect(
        verifyMysqlFixture(good.config, { policy }),
      ).resolves.toBeUndefined();
      const calls = (await Bun.file(good.calls).text())
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      expect(calls[0].argv).toEqual([
        "context",
        "inspect",
        "colima",
        "--format",
        "{{.Endpoints.docker.Host}}",
      ]);
      expect(calls.slice(1).map((call) => call.argv.at(-1))).toEqual([
        "hmail-cleanup-tests-mysql-1",
        "hmail-cleanup-tests-mysql-1",
      ]);
    } finally {
      await good.close();
    }
    for (const denied of [
      {
        change: { endpoint: "tcp://remote.example:2376" },
        reason: "local Docker context",
        calls: 1,
      },
      {
        change: { project: "production" },
        reason: "named disposable MySQL fixture",
        calls: 2,
      },
      {
        change: { binding: "0.0.0.0:13306" },
        reason: "verified loopback fixture port",
        calls: 3,
      },
    ]) {
      const f = await fixture(denied.change);
      try {
        await expect(verifyMysqlFixture(f.config, { policy })).rejects.toThrow(
          denied.reason,
        );
        expect(
          (await Bun.file(f.calls).text()).trim().split("\n"),
        ).toHaveLength(denied.calls);
      } finally {
        await f.close();
      }
    }
  },
);
