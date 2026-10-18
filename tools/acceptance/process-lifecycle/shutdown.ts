import { terminateProcessGroup } from "../../lib/process-group";
import { settleWithin } from "./deadline";
import type { ProcessCleanupOptions } from "./types";

export const shutdownProcess = async ({
  child,
  captures,
  policy,
}: ProcessCleanupOptions): Promise<readonly unknown[]> => {
  const errors: unknown[] = [];
  try {
    const group = await terminateProcessGroup({
      pid: child.pid,
      termGraceMs: policy.termGraceMs,
      killGraceMs: policy.killGraceMs,
    });
    if (!group.gone) errors.push(new Error("Owned process group did not exit"));
  } catch (error) {
    errors.push(error);
  }
  const completed = Promise.allSettled(captures.map((capture) => capture.done));
  const drained = await settleWithin(completed, policy.drainMs);
  if (drained.kind === "deadline") {
    const closed = await settleWithin(
      Promise.allSettled(captures.map((capture) => capture.close())),
      policy.drainMs,
    );
    if (closed.kind === "deadline")
      errors.push(
        new Error("Check pipes did not close within the grace period"),
      );
    else if (closed.result.status === "fulfilled")
      for (const result of closed.result.value)
        if (result.status === "rejected") errors.push(result.reason);
  }
  const captured = await settleWithin(completed, policy.drainMs);
  if (captured.kind === "deadline")
    errors.push(new Error("Check pipe readers did not settle"));
  else if (captured.result.status === "fulfilled")
    for (const result of captured.result.value)
      if (result.status === "rejected") errors.push(result.reason);
  const exited = await settleWithin(child.exited, policy.killGraceMs);
  if (exited.kind === "deadline")
    errors.push(new Error("Check child exit was not observed"));
  else if (exited.result.status === "rejected")
    errors.push(exited.result.reason);
  return errors;
};
