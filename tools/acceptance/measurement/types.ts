export type MeasurementOptions = Readonly<{
  sampleMs: number;
  windowMs: number;
}>;
export type OwnedFixtureProcess = Readonly<{
  child: Bun.Subprocess<"pipe", "pipe", "inherit">;
  temporaryRoot: string;
  signal: AbortSignal;
  retain: (release: () => Promise<void>) => Promise<void>;
  stop: () => Promise<void>;
}>;
export type MeasurementConfig = MeasurementOptions &
  Readonly<{
    root: string;
    output: string;
    centerBinary: string;
    fixtureBinary: string;
    staticDirectory: string;
    releaseManifest: string | undefined;
    buildReceipt: string;
  }>;
export type MeasurementBuild = Readonly<{
  format_version: 1;
  build: string;
  source: SourceIdentity;
  profile: "release";
  default_features: false;
  features: readonly ["center"];
  artifacts: readonly Artifact[];
}>;
export type ProcessSample = Readonly<{
  atMs: number;
  rssBytes: number;
  cpuMs: number;
}>;
export type ProcessSummary = Readonly<{
  samples: number;
  durationMs: number;
  rssMinBytes: number;
  rssMaxBytes: number;
  rssMeanBytes: number;
  cpuElapsedMs: number;
  cpuAveragePercentOfOneCore: number;
}>;
export type ProcessWindow = Readonly<{
  name: string;
  summary: ProcessSummary;
  raw: readonly ProcessSample[];
}>;
export type ScriptAsset = Readonly<{
  path: string;
  encodedBodyBytes: number;
  decodedBodyBytes: number;
  transferBytes: number;
}>;
export type BrowserSnapshot = Readonly<{
  name: string;
  domElements: number;
  domNodes: number;
  jsHeapUsedBytes: number;
  jsHeapTotalBytes: number;
  journalDomRows: number;
  historyDomRows: number;
  scripts: readonly ScriptAsset[];
}>;
import type { Artifact, SourceIdentity } from "../../release/types";
