import { captureStream } from "./capture";
import { shutdownProcess } from "./shutdown";
import type { CheckProcessOptions, ProcessOutcome } from "./types";

const describeFailure = (error: unknown) =>
  error instanceof Error ? error.message : "Unknown subprocess failure";

export const executeProcess = async ({
  argv,
  cwd,
  environment,
  output,
  policy,
  signal,
}: CheckProcessOptions): Promise<ProcessOutcome> => {
  if (process.platform === "win32")
    throw new Error(
      "Acceptance subprocess-group cleanup is unsupported on Windows",
    );
  if (signal?.aborted) return { kind: "stopped", reason: "Check cancelled" };
  const child = Bun.spawn([...argv], {
    cwd,
    env: environment,
    detached: true,
    stdout: "pipe",
    stderr: "pipe",
    stdin: "ignore",
  });
  const captures = [
    captureStream({ stream: child.stdout, write: output.stdout }),
    captureStream({ stream: child.stderr, write: output.stderr }),
  ];
  const completed = Promise.all([
    child.exited,
    ...captures.map((capture) => capture.done),
  ]).then(([code]): ProcessOutcome => ({ kind: "exited", code }));
  const stopped = Promise.withResolvers<ProcessOutcome>();
  const timer = setTimeout(
    () =>
      stopped.resolve({
        kind: "stopped",
        reason: `Check timed out after ${policy.timeoutMs} ms`,
      }),
    policy.timeoutMs,
  );
  const cancel = () =>
    stopped.resolve({ kind: "stopped", reason: "Check cancelled" });
  signal?.addEventListener("abort", cancel, { once: true });
  if (signal?.aborted) cancel();
  try {
    const outcome = await Promise.race([completed, stopped.promise]).then(
      (value) => ({ status: "fulfilled", value }) as const,
      (reason: unknown) => ({ status: "rejected", reason }) as const,
    );
    const cleanupErrors = await shutdownProcess({ child, captures, policy });
    if (outcome.status === "rejected")
      return {
        kind: "stopped",
        reason: [outcome.reason, ...cleanupErrors]
          .map(describeFailure)
          .join("; "),
      };
    if (cleanupErrors.length > 0) {
      const failures = cleanupErrors.map(describeFailure).join("; ");
      const reason =
        outcome.value.kind === "stopped"
          ? `${outcome.value.reason}; cleanup failed: ${failures}`
          : `Check cleanup failed: ${failures}`;
      return { kind: "stopped", reason };
    }
    return outcome.value;
  } finally {
    clearTimeout(timer);
    signal?.removeEventListener("abort", cancel);
  }
};
