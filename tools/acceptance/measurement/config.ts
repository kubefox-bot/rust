import { resolve } from "node:path";
import {
  CENTER_RELATIVE_PATH,
  DEFAULT_SAMPLE_MS,
  DEFAULT_WINDOW_MS,
  FIXTURE_RELATIVE_PATH,
  MAX_SAMPLE_MS,
  MAX_WINDOW_MS,
  MIN_SAMPLE_MS,
  MIN_WINDOW_MS,
  STATIC_RELATIVE_PATH,
} from "./conts";
import type { MeasurementConfig, MeasurementOptions } from "./types";
export const MEASUREMENT_OUTPUT = resolve(
  import.meta.dir,
  "../../../target/acceptance/resources.json",
);

const boundedInteger = (
  input: Readonly<{
    key: string;
    value: string | undefined;
    fallback: number;
    min: number;
    max: number;
  }>,
): number => {
  if (input.value === undefined) return input.fallback;
  if (!/^\d+$/.test(input.value))
    throw new Error(`${input.key} must be an integer`);
  const value = Number(input.value);
  if (!Number.isSafeInteger(value) || value < input.min || value > input.max)
    throw new Error(
      `${input.key} must be between ${input.min} and ${input.max}`,
    );
  return value;
};
export const readMeasurementOptions = (
  environment: Readonly<Record<string, string | undefined>>,
): MeasurementOptions => {
  const keys = new Set([
    "HMAIL_MVP_MEASURE_SAMPLE_MS",
    "HMAIL_MVP_MEASURE_WINDOW_MS",
  ]);
  for (const key of Object.keys(environment))
    if (key.startsWith("HMAIL_MVP_MEASURE_") && !keys.has(key))
      throw new Error(`Unknown measurement option: ${key}`);
  const sampleMs = boundedInteger({
    key: "HMAIL_MVP_MEASURE_SAMPLE_MS",
    value: environment.HMAIL_MVP_MEASURE_SAMPLE_MS,
    fallback: DEFAULT_SAMPLE_MS,
    min: MIN_SAMPLE_MS,
    max: MAX_SAMPLE_MS,
  });
  const windowMs = boundedInteger({
    key: "HMAIL_MVP_MEASURE_WINDOW_MS",
    value: environment.HMAIL_MVP_MEASURE_WINDOW_MS,
    fallback: DEFAULT_WINDOW_MS,
    min: MIN_WINDOW_MS,
    max: MAX_WINDOW_MS,
  });
  if (windowMs < sampleMs * 2)
    throw new Error(
      "Measurement window must allow at least three process samples",
    );
  return { sampleMs, windowMs };
};
export const readMeasurementConfig = (): MeasurementConfig => {
  const root = resolve(import.meta.dir, "../../..");
  const releaseManifest = Bun.env.HMAIL_MVP_RELEASE_MANIFEST;
  const manifestPath =
    releaseManifest === undefined ? undefined : resolve(root, releaseManifest);
  if (
    manifestPath &&
    !manifestPath.startsWith(`${resolve(root, "target/releases")}/`)
  )
    throw new Error(
      "Measurement release manifest must be within repository target/releases",
    );
  return {
    ...readMeasurementOptions(Bun.env),
    root,
    output: MEASUREMENT_OUTPUT,
    centerBinary: resolve(root, CENTER_RELATIVE_PATH),
    fixtureBinary: resolve(root, FIXTURE_RELATIVE_PATH),
    staticDirectory: resolve(root, STATIC_RELATIVE_PATH),
    releaseManifest: manifestPath,
    buildReceipt: resolve(root, "target/acceptance/measurement-build.json"),
  };
};
