import { mkdir, open, rm } from "node:fs/promises";
import { dirname } from "node:path";
import { readReleaseConfig } from "../release/config";
import { run } from "../lib/process";
import {
  MEASUREMENT_OUTPUT,
  readMeasurementConfig,
} from "./measurement/config";
import { readReleaseManifest } from "../release/manifest";
import { captureMeasurementBuild } from "./measurement/build-receipt";
import { PRIVATE_REPORT_MODE } from "./measurement/conts";
import { writeMeasurementFailure } from "./measurement/report";

try {
  await writeMeasurementFailure({
    config: { output: MEASUREMENT_OUTPUT },
    reason:
      "Measurement preparation is in progress; previous PASS is invalidated",
  });
  const measurement = readMeasurementConfig();
  await rm(measurement.buildReceipt, { force: true });
  const config = await readReleaseConfig();
  const manifest = measurement.releaseManifest
    ? await readReleaseManifest(measurement.releaseManifest)
    : undefined;
  if (manifest && manifest.source_fingerprint !== config.source.fingerprint)
    throw new Error(
      "Release source differs from the native measurement source",
    );
  // Preserve the frozen artifact identity if the same source was committed later.
  const source = manifest
    ? {
        revision: manifest.source_revision,
        fingerprint: manifest.source_fingerprint,
        dirty: manifest.dirty,
      }
    : config.source;
  const build = manifest?.build ?? config.build;
  const environment = {
    ...config.environment,
    HMAIL_BUILD_VERSION: build,
    HMAIL_SOURCE_REVISION: source.revision,
    PUBLIC_BUILD_VERSION: build,
  };
  for (const argv of [
    [
      "cargo",
      "build",
      "--release",
      "-p",
      "hmail-cleaner",
      "--no-default-features",
      "--features",
      "center",
      "--locked",
    ],
    [
      "cargo",
      "build",
      "--release",
      "-p",
      "hmail-cleaner-center",
      "--example",
      "resource_fixture",
      "--locked",
    ],
    ["bun", "run", "web:build"],
  ])
    await run({ argv, cwd: config.root, env: environment, inherit: true });
  const receipt = await captureMeasurementBuild({
    config: measurement,
    build,
    source,
  });
  await mkdir(dirname(measurement.buildReceipt), { recursive: true });
  const file = await open(measurement.buildReceipt, "wx", PRIVATE_REPORT_MODE);
  try {
    await file.writeFile(`${JSON.stringify(receipt, null, 2)}\n`);
    await file.sync();
  } finally {
    await file.close();
  }
  // measure.ts owns final validation and the atomic report PASS boundary.
  await run({
    argv: ["bun", "tools/acceptance/measure.ts"],
    cwd: config.root,
    env: environment,
    inherit: true,
  });
} catch (error) {
  await writeMeasurementFailure({
    config: { output: MEASUREMENT_OUTPUT },
    reason:
      error instanceof Error ? error.message : "Measurement preparation failed",
  });
  throw error;
}
