import type { ProcessSample, ProcessSummary } from "./types";
const KIB_BYTES = 1024;
const SECOND_MS = 1000;
const MINUTE_SECONDS = 60;
const HOUR_SECONDS = 3600;
const DAY_SECONDS = 86400;
const cpuTimeMs = (value: string): number => {
  const match = /^(?:(\d+)-)?(?:(\d+):)?(\d+):(\d{2})(?:\.(\d{1,3}))?$/.exec(
    value,
  );
  if (!match) throw new Error("Invalid cumulative process CPU time");
  const [
    ,
    days = "0",
    hours = "0",
    minutes = "0",
    seconds = "0",
    fraction = "0",
  ] = match;
  if (
    (Number(minutes) >= MINUTE_SECONDS && match[2] !== undefined) ||
    Number(seconds) >= MINUTE_SECONDS
  )
    throw new Error("Invalid cumulative process CPU time");
  return (
    (Number(days) * DAY_SECONDS +
      Number(hours) * HOUR_SECONDS +
      Number(minutes) * MINUTE_SECONDS +
      Number(seconds)) *
      SECOND_MS +
    Number(fraction.padEnd(3, "0"))
  );
};
export const parseProcessRow = (
  input: Readonly<{ output: string; atMs: number }>,
): ProcessSample => {
  const match = /^\s*(\d+)\s+(\S+)\s*$/.exec(input.output);
  if (!match?.[1] || !match[2])
    throw new Error("Missing or invalid process counters");
  const rssBytes = Number(match[1]) * KIB_BYTES;
  const cpuMs = cpuTimeMs(match[2]);
  if (
    !Number.isSafeInteger(rssBytes) ||
    !Number.isSafeInteger(cpuMs) ||
    !Number.isFinite(input.atMs)
  )
    throw new Error("Process counters exceed supported precision");
  return { atMs: input.atMs, rssBytes, cpuMs };
};
export const summarizeSamples = (
  samples: readonly ProcessSample[],
): ProcessSummary => {
  const first = samples[0];
  const last = samples.at(-1);
  if (samples.length < 2 || !first || !last)
    throw new Error("At least two process samples are required");
  for (const [index, sample] of samples.entries()) {
    const previous = samples[index - 1];
    if (
      ![sample.atMs, sample.rssBytes, sample.cpuMs].every(Number.isFinite) ||
      sample.rssBytes < 0 ||
      sample.cpuMs < 0 ||
      (previous &&
        (sample.atMs <= previous.atMs || sample.cpuMs < previous.cpuMs))
    )
      throw new Error("Invalid or regressing process samples");
  }
  const durationMs = last.atMs - first.atMs;
  const cpuElapsedMs = last.cpuMs - first.cpuMs;
  return {
    samples: samples.length,
    durationMs,
    rssMinBytes: Math.min(...samples.map((sample) => sample.rssBytes)),
    rssMaxBytes: Math.max(...samples.map((sample) => sample.rssBytes)),
    rssMeanBytes:
      samples.reduce((total, sample) => total + sample.rssBytes, 0) /
      samples.length,
    cpuElapsedMs,
    cpuAveragePercentOfOneCore: (cpuElapsedMs / durationMs) * 100,
  };
};
