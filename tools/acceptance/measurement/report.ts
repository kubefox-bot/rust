import { mkdir, open, rename, rm } from "node:fs/promises";
import { dirname } from "node:path";
import type { MeasurementBuild, MeasurementConfig } from "./types";
import { validateMeasurementBuild } from "./build-receipt";
import { PRIVATE_REPORT_MODE } from "./conts";
const writeReport = async (
  input: Readonly<{
    output: string;
    report: Readonly<Record<string, unknown>>;
    beforeCommit?: () => Promise<void>;
  }>,
): Promise<void> => {
  await mkdir(dirname(input.output), { recursive: true });
  const staged = `${input.output}.${crypto.randomUUID()}.new`;
  const file = await open(staged, "wx", PRIVATE_REPORT_MODE);
  try {
    try {
      await file.writeFile(`${JSON.stringify(input.report, null, 2)}\n`);
      await file.sync();
    } finally {
      await file.close();
    }
    await input.beforeCommit?.();
    await rename(staged, input.output);
  } finally {
    await rm(staged, { force: true });
  }
};
export const writeMeasurementFailure = async (
  input: Readonly<{
    config: Pick<MeasurementConfig, "output">;
    reason: string;
    details?: Readonly<Record<string, unknown>>;
  }>,
): Promise<void> => {
  await writeReport({
    output: input.config.output,
    report: {
      ...input.details,
      formatVersion: 1,
      verdict: "FAIL",
      finishedAt: new Date().toISOString(),
      reason: input.reason,
    },
  });
};
export const commitMeasurementReport = async (
  input: Readonly<{
    config: MeasurementConfig;
    build: MeasurementBuild;
    report: Readonly<Record<string, unknown>>;
    runtime: Readonly<{ center: string; web: string }>;
  }>,
): Promise<void> => {
  await writeMeasurementFailure({
    config: input.config,
    reason: "Measurement report validation is in progress",
  });
  try {
    await writeReport({
      output: input.config.output,
      report: {
        ...input.report,
        verdict: "PASS",
        source: input.build.source,
        buildProvenance: input.build,
      },
      beforeCommit: async () => {
        await validateMeasurementBuild({
          config: input.config,
          expected: input.build,
          runtimeBuild: input.runtime.center,
          webBuild: input.runtime.web,
        });
      },
    });
  } catch (error) {
    await writeMeasurementFailure({
      config: input.config,
      reason:
        error instanceof Error
          ? error.message
          : "Measurement provenance validation failed",
      details: { source: input.build.source },
    });
    throw error;
  }
};
