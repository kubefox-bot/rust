import { expect, test } from "bun:test";
import { lstat, mkdir, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { executeCheck } from "../execute-check";
import type { AcceptanceConfig } from "../types";

const posixTest = process.platform === "win32" ? test.skip : test;
const policy = {
  timeoutMs: 300,
  termGraceMs: 50,
  killGraceMs: 500,
  drainMs: 200,
};
const bounded = async <T>(pending: Promise<T>): Promise<T> => {
  const deadline = Promise.withResolvers<never>();
  const timer = setTimeout(
    () =>
      deadline.reject(new Error("Check did not produce its bounded receipt")),
    1800,
  );
  try {
    return await Promise.race([pending, deadline.promise]);
  } finally {
    clearTimeout(timer);
  }
};
const exists = (pid: number) => {
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
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (await Bun.file(path).exists())
      return Number(await Bun.file(path).text());
    await Bun.sleep(10);
  }
  throw new Error("Synthetic process did not start");
};
const fixture = async () => {
  const root = await mkdtemp(join(tmpdir(), "hmail-check-process-"));
  const pidFile = join(root, "child.pid");
  const config: AcceptanceConfig = {
    root,
    output: root,
    database: false,
    deliveryManifest: undefined,
    dockerContext: "colima",
    environment: {},
  };
  return {
    root,
    pidFile,
    config,
    close: async () => {
      if (await Bun.file(pidFile).exists()) {
        const pid = Number(await Bun.file(pidFile).text());
        if (exists(pid)) process.kill(pid, "SIGKILL");
      }
      await rm(root, { recursive: true, force: true });
    },
  };
};

posixTest(
  "a shorter rerun replaces the same check log without an old failure tail",
  async () => {
    const f = await fixture();
    const log = join(f.root, "repeated.log");
    try {
      const failed = await executeCheck(
        f.config,
        {
          name: "repeated",
          argv: [
            process.execPath,
            "-e",
            "process.stdout.write('first-run-old-content-tail\\n');process.exit(7);",
          ],
        },
        { policy },
      );
      expect(failed.verdict).toBe("FAIL");
      expect(await Bun.file(log).text()).toBe("first-run-old-content-tail\n");
      expect((await lstat(log)).mode & 0o777).toBe(0o600);
      const passed = await executeCheck(
        f.config,
        {
          name: "repeated",
          argv: [process.execPath, "-e", "process.stdout.write('new\\n');"],
        },
        { policy },
      );
      expect(passed.verdict).toBe("PASS");
      expect(passed.log).toBe(failed.log);
      expect(await Bun.file(log).text()).toBe("new\n");
      expect((await lstat(log)).mode & 0o777).toBe(0o600);
    } finally {
      await f.close();
    }
  },
);

posixTest(
  "an unusable log returns its original I/O failure without starting a child",
  async () => {
    const f = await fixture();
    const started = join(f.root, "must-not-start");
    try {
      await mkdir(join(f.root, "unusable.log"));
      const receipt = await executeCheck(
        f.config,
        {
          name: "unusable",
          argv: [
            process.execPath,
            "-e",
            `await Bun.write(${JSON.stringify(started)}, 'started');`,
          ],
        },
        { policy },
      );
      expect(receipt.verdict).toBe("FAIL");
      expect(receipt.reason).toContain("EISDIR");
      expect(await Bun.file(started).exists()).toBe(false);
    } finally {
      await f.close();
    }
  },
);

posixTest(
  "a check ignoring TERM times out with a FAIL receipt and no live child",
  async () => {
    const f = await fixture();
    const script = `process.on("SIGTERM", () => {}); await Bun.write(${JSON.stringify(f.pidFile)}, String(process.pid)); console.log("started"); console.error("error-started"); setInterval(() => {}, 1000);`;
    const pending = executeCheck(
      f.config,
      { name: "stubborn", argv: [process.execPath, "-e", script] },
      { policy },
    );
    try {
      const pid = await waitForPid(f.pidFile);
      const receipt = await bounded(pending);
      expect(receipt.verdict).toBe("FAIL");
      expect(receipt.reason).toContain("timed out");
      expect(exists(pid)).toBe(false);
      const log = await Bun.file(join(f.root, "stubborn.log")).text();
      expect(log).toContain("started");
      expect(log).toContain("error-started");
    } finally {
      await f.close();
      await pending;
    }
  },
);

posixTest(
  "an exited parent cannot leave a descendant holding the check pipes open",
  async () => {
    const f = await fixture();
    const descendant = `process.on("SIGTERM", () => {}); await Bun.write(${JSON.stringify(f.pidFile)}, String(process.pid)); console.log("descendant-ready"); setInterval(() => {}, 1000);`;
    const script = `const child = Bun.spawn([process.execPath, "-e", ${JSON.stringify(descendant)}], { stdout: "inherit", stderr: "inherit" }); child.unref(); process.exit(0);`;
    const pending = executeCheck(
      f.config,
      { name: "orphan-pipes", argv: [process.execPath, "-e", script] },
      { policy },
    );
    try {
      const pid = await waitForPid(f.pidFile);
      const receipt = await bounded(pending);
      expect(receipt.verdict).toBe("FAIL");
      expect(receipt.reason).toContain("timed out");
      expect(exists(pid)).toBe(false);
      expect(await Bun.file(join(f.root, "orphan-pipes.log")).text()).toContain(
        "descendant-ready",
      );
    } finally {
      await f.close();
      await pending;
    }
  },
);

posixTest(
  "cancellation terminates the owned process before returning a FAIL receipt",
  async () => {
    const f = await fixture();
    const controller = new AbortController();
    const script = `process.on("SIGTERM", () => {}); await Bun.write(${JSON.stringify(f.pidFile)}, String(process.pid)); setInterval(() => {}, 1000);`;
    const pending = executeCheck(
      f.config,
      { name: "cancelled", argv: [process.execPath, "-e", script] },
      { policy: { ...policy, timeoutMs: 10000 }, signal: controller.signal },
    );
    try {
      const pid = await waitForPid(f.pidFile);
      controller.abort();
      const receipt = await bounded(pending);
      expect(receipt.verdict).toBe("FAIL");
      expect(receipt.reason).toContain("cancelled");
      expect(exists(pid)).toBe(false);
    } finally {
      await f.close();
      await pending;
    }
  },
);

posixTest(
  "a timeout stays FAIL when TERM makes the process exit successfully",
  async () => {
    const f = await fixture();
    try {
      const receipt = await bounded(
        executeCheck(
          f.config,
          {
            name: "cooperative",
            argv: [
              process.execPath,
              "-e",
              'process.on("SIGTERM", () => process.exit(0)); setInterval(() => {}, 1000);',
            ],
          },
          { policy },
        ),
      );
      expect(receipt.verdict).toBe("FAIL");
      expect(receipt.reason).toContain("timed out");
    } finally {
      await f.close();
    }
  },
);

posixTest(
  "completed checks retain their actual exit verdict and both log streams",
  async () => {
    const f = await fixture();
    try {
      for (const code of [0, 7]) {
        const receipt = await executeCheck(
          f.config,
          {
            name: `exit-${code}`,
            argv: [
              process.execPath,
              "-e",
              `console.log("stdout-value"); console.error("stderr-value"); process.exit(${code});`,
            ],
          },
          { policy },
        );
        expect(receipt.verdict).toBe(code === 0 ? "PASS" : "FAIL");
        expect(receipt.reason).toBeUndefined();
        const log = await Bun.file(join(f.root, `exit-${code}.log`)).text();
        expect(log).toContain("stdout-value");
        expect(log).toContain("stderr-value");
      }
    } finally {
      await f.close();
    }
  },
);

posixTest(
  "a cancelled acceptance run skips remaining work and writes its failure report",
  async () => {
    const f = await fixture();
    const priorExitCode = process.exitCode;
    const controller = new AbortController();
    controller.abort();
    try {
      const { runAcceptance } = await import("../run");
      const report = await runAcceptance(
        {
          ...f.config,
          database: true,
          deliveryManifest: join(f.root, "missing-release.json"),
        },
        { signal: controller.signal },
      );
      const scheduled = report.checks.filter(
        (receipt) => receipt.log !== undefined,
      );
      expect(scheduled).toEqual([]);
      expect(report.checks).toContainEqual({
        name: "acceptance-cancelled",
        verdict: "FAIL",
        reason: "Acceptance cancelled",
      });
      const written = await Bun.file(join(f.root, "report.json")).json();
      expect(written.checks).toEqual(report.checks);
    } finally {
      process.exitCode = priorExitCode ?? 0;
      await f.close();
    }
  },
);

posixTest(
  "pipe draining is bounded when an external process retains inherited handles",
  async () => {
    const f = await fixture();
    const external = `process.on("SIGTERM", () => {}); await Bun.write(${JSON.stringify(f.pidFile)}, String(process.pid)); console.log("external-ready"); setInterval(() => {}, 1000);`;
    const script = `const child = Bun.spawn([process.execPath, "-e", ${JSON.stringify(external)}], { detached: true, stdout: "inherit", stderr: "inherit" }); child.unref(); process.exit(0);`;
    const pending = executeCheck(
      f.config,
      { name: "external-pipe", argv: [process.execPath, "-e", script] },
      { policy },
    );
    try {
      const pid = await waitForPid(f.pidFile);
      const receipt = await bounded(pending);
      expect(receipt.verdict).toBe("FAIL");
      expect(receipt.reason).toContain("timed out");
      expect(exists(pid)).toBe(true);
      expect(
        await Bun.file(join(f.root, "external-pipe.log")).text(),
      ).toContain("external-ready");
    } finally {
      await f.close();
      await pending;
    }
  },
);
