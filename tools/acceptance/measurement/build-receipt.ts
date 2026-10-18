import Ajv from "ajv";
import { lstat } from "node:fs/promises";
import { relative } from "node:path";
import { artifact, readReleaseManifest } from "../../release/manifest";
import { sourceIdentity } from "../../release/source-identity";
import type { SourceIdentity } from "../../release/types";
import type { MeasurementBuild, MeasurementConfig } from "./types";
import {
  BUILD_RECEIPT_SCHEMA,
  CENTER_RELATIVE_PATH,
  FIXTURE_RELATIVE_PATH,
  MAX_BUILD_RECEIPT_BYTES,
} from "./conts";
import { nativeArtifact, staticArtifacts } from "./artifacts";
const validateReceipt = new Ajv({ strict: true }).compile<MeasurementBuild>(
  BUILD_RECEIPT_SCHEMA,
);
const assertReceipt = (value: unknown): MeasurementBuild => {
  if (!validateReceipt(value))
    throw new Error("Invalid measurement build receipt contract");
  const dirtySuffix = `-dirty.${value.source.fingerprint.slice(0, 12)}`;
  if (
    value.source.dirty
      ? !value.source.revision.endsWith(dirtySuffix)
      : value.source.revision.includes("-dirty")
  )
    throw new Error(
      "Measurement build receipt has inconsistent source identity",
    );
  return value;
};
const actualArtifacts = async (config: MeasurementConfig) => {
  const center = await nativeArtifact({
    root: config.root,
    relative: CENTER_RELATIVE_PATH,
  });
  const fixture = await nativeArtifact({
    root: config.root,
    relative: FIXTURE_RELATIVE_PATH,
  });
  const files = (await staticArtifacts(config.staticDirectory))
    .map((file) => file.path)
    .sort();
  return Promise.all(
    [center, fixture, ...files].map((path) =>
      artifact({
        path,
        name: relative(config.root, path).replaceAll("\\", "/"),
      }),
    ),
  );
};
const assertSource = async (
  input: Readonly<{ config: MeasurementConfig; source: SourceIdentity }>,
): Promise<void> => {
  if (
    (await sourceIdentity(input.config.root)).fingerprint !==
    input.source.fingerprint
  )
    throw new Error(
      "Measurement source differs from the authoritative artifact source fingerprint",
    );
};
const assertManifest = async (
  input: Readonly<{ config: MeasurementConfig; receipt: MeasurementBuild }>,
): Promise<void> => {
  if (!input.config.releaseManifest) return;
  const manifest = await readReleaseManifest(input.config.releaseManifest);
  if (
    manifest.build !== input.receipt.build ||
    manifest.source_revision !== input.receipt.source.revision ||
    manifest.source_fingerprint !== input.receipt.source.fingerprint ||
    manifest.dirty !== input.receipt.source.dirty
  )
    throw new Error(
      "Release manifest source/build identity differs from native measurement build provenance",
    );
};
export const captureMeasurementBuild = async (
  input: Readonly<{
    config: MeasurementConfig;
    build: string;
    source: SourceIdentity;
  }>,
): Promise<MeasurementBuild> => {
  await assertSource(input);
  const receipt = assertReceipt({
    format_version: 1,
    build: input.build,
    source: input.source,
    profile: "release",
    default_features: false,
    features: ["center"],
    artifacts: await actualArtifacts(input.config),
  });
  await assertManifest({ config: input.config, receipt });
  await assertSource(input);
  return receipt;
};
export const validateMeasurementBuild = async (
  input: Readonly<{
    config: MeasurementConfig;
    expected?: MeasurementBuild;
    runtimeBuild?: string;
    webBuild?: string;
  }>,
): Promise<MeasurementBuild> => {
  const metadata = await lstat(input.config.buildReceipt).catch(
    (error: unknown) => {
      throw new Error(
        "An authoritative pre-measurement build receipt is required; run mise run measure:mvp",
        { cause: error },
      );
    },
  );
  if (!metadata.isFile() || metadata.size > MAX_BUILD_RECEIPT_BYTES)
    throw new Error("Invalid measurement build receipt file");
  const receipt = assertReceipt(
    await Bun.file(input.config.buildReceipt).json(),
  );
  if (
    input.expected &&
    JSON.stringify(input.expected) !== JSON.stringify(receipt)
  )
    throw new Error("Measurement build receipt changed during the run");
  if (
    (input.runtimeBuild !== undefined &&
      input.runtimeBuild !== receipt.build) ||
    (input.webBuild !== undefined && input.webBuild !== receipt.build)
  )
    throw new Error(
      "Native/web runtime build differs from the authoritative measurement build receipt",
    );
  await assertSource({ config: input.config, source: receipt.source });
  if (
    JSON.stringify(await actualArtifacts(input.config)) !==
    JSON.stringify(receipt.artifacts)
  )
    throw new Error(
      "Measurement artifact bytes differ from the authoritative build receipt",
    );
  await assertManifest({ config: input.config, receipt });
  await assertSource({ config: input.config, source: receipt.source });
  return receipt;
};
