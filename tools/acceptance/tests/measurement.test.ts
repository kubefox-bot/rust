import { expect, spyOn, test } from "bun:test";
import { mkdtemp, mkdir, realpath, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { nativeArtifact } from "../measurement/artifacts";
import { parseProcessRow, summarizeSamples } from "../measurement/counters";
import { readMeasurementOptions } from "../measurement/config";
import { additionalScripts } from "../measurement/browser-assets";
import { ownFixtureProcess } from "../measurement/fixture-process";
import { lineReader } from "../../../apps/web/tests/browser/line-reader";
import { run } from "../../lib/process";
import { sourceIdentity } from "../../release/source-identity";
import {
  captureMeasurementBuild,
  validateMeasurementBuild,
} from "../measurement/build-receipt";
import { commitMeasurementReport } from "../measurement/report";
import type { MeasurementConfig } from "../measurement/types";
import { terminateProcessGroup } from "../../lib/process-group";
const confirmedAbsentGroup = (
  signal: typeof process.kill,
  pid: number,
): boolean => {
  try {
    signal(-pid, 0);
    return false;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ESRCH")
      return true;
    throw error;
  }
};

const stubbornGroup = async () => {
  const owned = await ownFixtureProcess({
    launch: () =>
      Bun.spawn(
        [
          process.execPath,
          "-e",
          "process.on('SIGTERM',()=>{});console.log(JSON.stringify({ready:true}));setInterval(()=>{},50)",
        ],
        { detached: true, stdin: "pipe", stdout: "pipe", stderr: "inherit" },
      ),
    exitGraceMs: 20,
    termGraceMs: 20,
    killGraceMs: 75,
  });
  const output = lineReader(owned.child.stdout);
  await output.line();
  return { owned, output };
};

test("a transient post-KILL EPERM liveness probe retries until actual ESRCH before cleanup succeeds", async () => {
  const { owned, output } = await stubbornGroup();
  const original = process.kill.bind(process);
  const denied = Object.assign(
    new Error("Synthetic transient liveness EPERM"),
    { code: "EPERM" },
  );
  let killed = false;
  let deniedProbes = 0;
  const signal = spyOn(process, "kill").mockImplementation((pid, kind) => {
    if (
      pid === -owned.child.pid &&
      kind === 0 &&
      killed &&
      deniedProbes === 0
    ) {
      deniedProbes++;
      throw denied;
    }
    const result = original(pid, kind);
    if (pid === -owned.child.pid && kind === "SIGKILL") killed = true;
    return result;
  });
  try {
    await Bun.write(join(owned.temporaryRoot, "private-data"), "fixture-only");
    await owned.stop();
    expect(deniedProbes).toBe(1);
    expect(confirmedAbsentGroup(original, owned.child.pid)).toBe(true);
    expect(
      await Bun.file(join(owned.temporaryRoot, "private-data")).exists(),
    ).toBe(false);
  } finally {
    signal.mockRestore();
    await Promise.allSettled([owned.stop()]);
    await output.close();
    if (confirmedAbsentGroup(original, owned.child.pid))
      await rm(owned.temporaryRoot, { recursive: true, force: true });
  }
}, 5000);

test("persistent post-KILL EPERM stays uncertain with its cause and retains private files", async () => {
  const { owned, output } = await stubbornGroup();
  const original = process.kill.bind(process);
  const denied = Object.assign(
    new Error("Synthetic persistent liveness EPERM"),
    { code: "EPERM" },
  );
  let killed = false;
  let deniedProbes = 0;
  const signal = spyOn(process, "kill").mockImplementation((pid, kind) => {
    if (pid === -owned.child.pid && kind === 0 && killed) {
      deniedProbes++;
      throw denied;
    }
    const result = original(pid, kind);
    if (pid === -owned.child.pid && kind === "SIGKILL") killed = true;
    return result;
  });
  try {
    await Bun.write(join(owned.temporaryRoot, "private-data"), "fixture-only");
    const failure: unknown = await owned
      .stop()
      .catch((error: unknown) => error);
    expect(failure).toBeInstanceOf(AggregateError);
    if (!(failure instanceof AggregateError))
      throw new Error("Expected a preserved cleanup failure");
    const cause: unknown = failure.errors[0];
    expect(cause).toBeInstanceOf(Error);
    if (!(cause instanceof Error))
      throw new Error("Expected a preserved liveness failure");
    expect(cause.cause).toBe(denied);
    expect(deniedProbes).toBeGreaterThan(1);
    expect(
      await Bun.file(join(owned.temporaryRoot, "private-data")).exists(),
    ).toBe(true);
  } finally {
    signal.mockRestore();
    await Promise.allSettled([owned.stop()]);
    await output.close();
    expect(confirmedAbsentGroup(original, owned.child.pid)).toBe(true);
    await rm(owned.temporaryRoot, { recursive: true });
  }
}, 5000);

test("an actual TERM permission failure propagates unchanged and never claims the group is gone", async () => {
  const { owned, output } = await stubbornGroup();
  const original = process.kill.bind(process);
  const denied = Object.assign(new Error("Synthetic TERM permission EPERM"), {
    code: "EPERM",
  });
  const signal = spyOn(process, "kill").mockImplementation((pid, kind) => {
    if (pid === -owned.child.pid && kind === "SIGTERM") throw denied;
    return original(pid, kind);
  });
  try {
    await expect(
      terminateProcessGroup({
        pid: owned.child.pid,
        termGraceMs: 20,
        killGraceMs: 75,
      }),
    ).rejects.toBe(denied);
    expect(original(owned.child.pid, 0)).toBe(true);
  } finally {
    signal.mockRestore();
    await owned.stop();
    await output.close();
  }
}, 5000);

const measurementRepository = async () => {
  const root = await realpath(
    await mkdtemp(join(tmpdir(), "hmail-measure-provenance-")),
  );
  await mkdir(join(root, "crates/example/src"), { recursive: true });
  await mkdir(join(root, "target/release/examples"), { recursive: true });
  await mkdir(join(root, "target/acceptance"), { recursive: true });
  await mkdir(join(root, "apps/web/dist"), { recursive: true });
  await Bun.write(join(root, ".gitignore"), "/target/\n/apps/web/dist/\n");
  await Bun.write(
    join(root, "crates/example/src/lib.rs"),
    "pub const VALUE: u8 = 1;\n",
  );
  await run({ argv: ["git", "init", "--quiet"], cwd: root });
  await run({ argv: ["git", "add", "."], cwd: root });
  await run({
    argv: [
      "git",
      "-c",
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "commit",
      "--quiet",
      "-m",
      "test: seed private measurement fixture",
    ],
    cwd: root,
  });
  const config: MeasurementConfig = {
    root,
    output: join(root, "target/acceptance/resources.json"),
    buildReceipt: join(root, "target/acceptance/measurement-build.json"),
    centerBinary: join(root, "target/release/hmail-cleaner"),
    fixtureBinary: join(root, "target/release/examples/resource_fixture"),
    staticDirectory: join(root, "apps/web/dist"),
    releaseManifest: undefined,
    sampleMs: 50,
    windowMs: 3000,
  };
  await Bun.write(config.centerBinary, "fixture native artifact");
  await Bun.write(config.fixtureBinary, "fixture runner artifact");
  await Bun.write(
    join(config.staticDirectory, "index.html"),
    "fixture static artifact",
  );
  return config;
};
test("measurement requires prebuild provenance and retains the dirty artifact identity after an unchanged source commit", async () => {
  const config = await measurementRepository();
  try {
    await expect(validateMeasurementBuild({ config })).rejects.toThrow(
      "receipt",
    );
    await Bun.write(
      join(config.root, "crates/example/src/lib.rs"),
      "pub const VALUE: u8 = 2;\n",
    );
    const source = await sourceIdentity(config.root);
    const build = await captureMeasurementBuild({
      config,
      build: "0.1.0+fixture.1",
      source,
    });
    await Bun.write(config.buildReceipt, JSON.stringify(build));
    await run({ argv: ["git", "add", "."], cwd: config.root });
    await run({
      argv: [
        "git",
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "commit",
        "--quiet",
        "-m",
        "test: commit unchanged measured source",
      ],
      cwd: config.root,
    });
    expect((await sourceIdentity(config.root)).dirty).toBe(false);
    expect(
      (
        await validateMeasurementBuild({
          config,
          runtimeBuild: build.build,
          webBuild: build.build,
        })
      ).source,
    ).toEqual(source);
    expect(source.dirty).toBe(true);
    await commitMeasurementReport({
      config,
      build,
      runtime: { center: build.build, web: build.build },
      report: { measured: true },
    });
    expect(await Bun.file(config.output).json()).toMatchObject({
      verdict: "PASS",
      source,
    });
    await expect(
      validateMeasurementBuild({ config, runtimeBuild: "another-build" }),
    ).rejects.toThrow("build");
    await Bun.write(config.centerBinary, "replaced native artifact");
    await expect(validateMeasurementBuild({ config })).rejects.toThrow(
      "artifact",
    );
  } finally {
    await rm(config.root, { recursive: true });
  }
});

test("source mutation invalidates an old PASS inside the atomic measurement report commit", async () => {
  const config = await measurementRepository();
  try {
    const source = await sourceIdentity(config.root);
    const build = await captureMeasurementBuild({
      config,
      build: "0.1.0+fixture.1",
      source,
    });
    await Bun.write(config.buildReceipt, JSON.stringify(build));
    await Bun.write(
      config.output,
      JSON.stringify({ verdict: "PASS", stale: true }),
    );
    await Bun.write(
      join(config.root, "crates/example/src/lib.rs"),
      "pub const VALUE: u8 = 99;\n",
    );
    await expect(
      commitMeasurementReport({
        config,
        build,
        runtime: { center: build.build, web: build.build },
        report: { measured: true },
      }),
    ).rejects.toThrow("source");
    const report: unknown = await Bun.file(config.output).json();
    expect(report).toMatchObject({ verdict: "FAIL" });
    expect(report).not.toMatchObject({ stale: true });
  } finally {
    await rm(config.root, { recursive: true });
  }
});

const runningPid = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ESRCH")
      return false;
    throw error;
  }
};
test("owned fixture cleanup kills a stubborn real descendant after its leader exits and removes its private files", async () => {
  const script = `const child=Bun.spawn([process.execPath,'-e',"process.on('SIGTERM',()=>{});setInterval(()=>{},50)"],{stdin:'ignore',stdout:'ignore',stderr:'inherit'});await Bun.sleep(100);console.log(JSON.stringify({pid:child.pid}));for await(const _ of Bun.stdin.stream()){};process.exit(0);`;
  const owned = await ownFixtureProcess({
    launch: () =>
      Bun.spawn([process.execPath, "-e", script], {
        detached: true,
        stdin: "pipe",
        stdout: "pipe",
        stderr: "inherit",
      }),
    exitGraceMs: 100,
    termGraceMs: 100,
    killGraceMs: 2000,
  });
  const output = lineReader(owned.child.stdout);
  let descendant = 0;
  try {
    const value = await output.line();
    if (
      typeof value !== "object" ||
      value === null ||
      !("pid" in value) ||
      typeof value.pid !== "number"
    )
      throw new Error("Invalid test descendant metadata");
    descendant = value.pid;
    await Bun.write(
      join(owned.temporaryRoot, "synthetic-secret"),
      "fixture-only",
    );
    expect(runningPid(descendant)).toBe(true);
    await owned.stop();
    expect(runningPid(descendant)).toBe(false);
    expect(
      await Bun.file(join(owned.temporaryRoot, "synthetic-secret")).exists(),
    ).toBe(false);
    await owned.stop();
  } finally {
    await owned.stop();
    await output.close();
    if (descendant && runningPid(descendant))
      process.kill(descendant, "SIGKILL");
  }
}, 10000);

test("fixture startup failure closes retained resources and removes the temporary root before returning failure", async () => {
  const owned = await ownFixtureProcess({
    launch: () =>
      Bun.spawn(
        [
          process.execPath,
          "-e",
          "process.on('SIGTERM',()=>{});setInterval(()=>{},50)",
        ],
        { detached: true, stdin: "pipe", stdout: "pipe", stderr: "inherit" },
      ),
    exitGraceMs: 50,
    termGraceMs: 100,
    killGraceMs: 2000,
  });
  let released = false;
  try {
    await Bun.write(join(owned.temporaryRoot, "fixture-data"), "private");
    await owned.retain(async () => {
      released = true;
    });
    const startup = async () => {
      try {
        throw new Error("synthetic startup failure");
      } finally {
        await owned.stop();
      }
    };
    await expect(startup()).rejects.toThrow("synthetic startup failure");
    expect(released).toBe(true);
    expect(runningPid(owned.child.pid)).toBe(false);
    expect(
      await Bun.file(join(owned.temporaryRoot, "fixture-data")).exists(),
    ).toBe(false);
  } finally {
    await owned.stop();
  }
}, 10000);

test("external SIGTERM cancellation cleans the real fixture group and its private data", async () => {
  const module = join(import.meta.dir, "../measurement/fixture-process.ts");
  const script = `import{ownFixtureProcess}from ${JSON.stringify(module)};const owned=await ownFixtureProcess({launch:()=>Bun.spawn([process.execPath,'-e',"process.on('SIGTERM',()=>{});setInterval(()=>{},50)"],{detached:true,stdin:'pipe',stdout:'pipe',stderr:'inherit'}),exitGraceMs:50,termGraceMs:100,killGraceMs:2000});await Bun.write(owned.temporaryRoot+'/private-data','fixture');console.log(JSON.stringify({pid:owned.child.pid,root:owned.temporaryRoot}));await new Promise(resolve=>owned.signal.addEventListener('abort',resolve,{once:true}));await owned.stop();`;
  const driver = Bun.spawn([process.execPath, "-e", script], {
    stdin: "ignore",
    stdout: "pipe",
    stderr: "inherit",
  });
  const output = lineReader(driver.stdout);
  let fixturePid = 0;
  try {
    const value = await output.line();
    if (
      typeof value !== "object" ||
      value === null ||
      !("pid" in value) ||
      typeof value.pid !== "number" ||
      !("root" in value) ||
      typeof value.root !== "string"
    )
      throw new Error("Invalid cancellation fixture metadata");
    fixturePid = value.pid;
    expect(runningPid(fixturePid)).toBe(true);
    driver.kill("SIGTERM");
    expect(await driver.exited).toBe(143);
    expect(runningPid(fixturePid)).toBe(false);
    expect(await Bun.file(join(value.root, "private-data")).exists()).toBe(
      false,
    );
  } finally {
    if (driver.exitCode === null) driver.kill("SIGKILL");
    await driver.exited;
    await output.close();
    if (fixturePid && runningPid(fixturePid))
      process.kill(fixturePid, "SIGKILL");
  }
}, 10000);

test("process samples use KiB RSS and cumulative CPU time instead of lifetime CPU percent", () => {
  expect(parseProcessRow({ output: " 2048  1:02.50\n", atMs: 1000 })).toEqual({
    atMs: 1000,
    rssBytes: 2097152,
    cpuMs: 62500,
  });
  expect(
    parseProcessRow({ output: " 1024  1-02:03:04.25\n", atMs: 0 }).cpuMs,
  ).toBe(93784250);
  const result = summarizeSamples([
    { atMs: 1000, rssBytes: 1024, cpuMs: 400 },
    { atMs: 2000, rssBytes: 4096, cpuMs: 600 },
    { atMs: 3000, rssBytes: 2048, cpuMs: 650 },
  ]);
  expect(result).toEqual({
    samples: 3,
    durationMs: 2000,
    rssMinBytes: 1024,
    rssMaxBytes: 4096,
    rssMeanBytes: 2389.3333333333335,
    cpuElapsedMs: 250,
    cpuAveragePercentOfOneCore: 12.5,
  });
});

test("invalid process counters and insufficient or regressing samples fail explicitly", () => {
  for (const output of [
    "",
    "-1 00:00.10",
    "123 NaN",
    "123 00:99.10",
    "123 0:00 5",
    "12345678901234567890 0:00",
  ])
    expect(() => parseProcessRow({ output, atMs: 0 })).toThrow();
  expect(() => summarizeSamples([])).toThrow();
  expect(() =>
    summarizeSamples([{ atMs: 0, rssBytes: 1, cpuMs: 1 }]),
  ).toThrow();
  expect(() =>
    summarizeSamples([
      { atMs: 1, rssBytes: 1, cpuMs: 2 },
      { atMs: 2, rssBytes: 1, cpuMs: 1 },
    ]),
  ).toThrow();
});

test("resource options bound sampling cost and reject unknown measurement overrides", () => {
  expect(readMeasurementOptions({})).toEqual({ sampleMs: 50, windowMs: 3000 });
  expect(
    readMeasurementOptions({
      HMAIL_MVP_MEASURE_SAMPLE_MS: "100",
      HMAIL_MVP_MEASURE_WINDOW_MS: "5000",
    }),
  ).toEqual({ sampleMs: 100, windowMs: 5000 });
  for (const environment of [
    { HMAIL_MVP_MEASURE_SAMPLE_MS: "0" },
    { HMAIL_MVP_MEASURE_SAMPLE_MS: "NaN" },
    { HMAIL_MVP_MEASURE_WINDOW_MS: "100" },
    { HMAIL_MVP_MEASURE_WINDOW_MS: "1800000" },
    { HMAIL_MVP_MEASURE_BINARY: "/tmp/arbitrary" },
  ])
    expect(() => readMeasurementOptions(environment)).toThrow();
});

test("native artifacts cannot resolve to an executable outside their exact repository path", async () => {
  const root = await mkdtemp(join(tmpdir(), "hmail-measure-guard-"));
  try {
    await mkdir(join(root, "target/release"), { recursive: true });
    await Bun.write(join(root, "target/release/hmail-cleaner"), "fixture");
    expect(
      await nativeArtifact({ root, relative: "target/release/hmail-cleaner" }),
    ).toBe(join(await realpath(root), "target/release/hmail-cleaner"));
    await expect(
      nativeArtifact({ root, relative: "../outside" }),
    ).rejects.toThrow();
    await Bun.write(join(root, "outside"), "fixture");
    await symlink(
      join(root, "outside"),
      join(root, "target/release/resource_fixture"),
    );
    await expect(
      nativeArtifact({ root, relative: "target/release/resource_fixture" }),
    ).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true });
  }
});

test("lazy script differences count only newly requested script assets", () => {
  expect(
    additionalScripts({
      initial: ["/shared.js", "/cleanup.js"],
      selected: ["/shared.js", "/history.js", "/history.js"],
    }),
  ).toEqual(["/history.js"]);
});
