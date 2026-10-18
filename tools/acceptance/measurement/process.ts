import { parseProcessRow, summarizeSamples } from "./counters";
import type { MeasurementOptions, ProcessSample, ProcessWindow } from "./types";
import { run } from "../../lib/process";
const sampleProcess = async (
  input: Readonly<{ pid: number; root: string }>,
): Promise<ProcessSample> => {
  if (!Number.isSafeInteger(input.pid) || input.pid <= 0)
    throw new Error("Invalid owned process id");
  const output = await run({
    argv: ["ps", "-p", String(input.pid), "-o", "rss=", "-o", "time="],
    cwd: input.root,
  });
  return parseProcessRow({ output, atMs: performance.now() });
};
export const sampleWindow = async (
  input: Readonly<{
    name: string;
    pid: number;
    root: string;
    options: MeasurementOptions;
    operation: () => Promise<void>;
  }>,
): Promise<ProcessWindow> => {
  const first = await sampleProcess(input);
  const raw: ProcessSample[] = [first];
  const endAtMs = first.atMs + input.options.windowMs;
  let completed = false;
  const operation = async () => {
    try {
      await input.operation();
    } finally {
      completed = true;
    }
  };
  const collect = async () => {
    do {
      await Bun.sleep(input.options.sampleMs);
      raw.push(await sampleProcess(input));
    } while (!completed || performance.now() < endAtMs);
  };
  const results = await Promise.allSettled([operation(), collect()]);
  const failed = results.filter(
    (result): result is PromiseRejectedResult => result.status === "rejected",
  );
  if (failed.length > 0)
    throw new AggregateError(
      failed.map((result) => result.reason),
      "Resource measurement operation or process sampling failed",
    );
  return { name: input.name, summary: summarizeSamples(raw), raw };
};
