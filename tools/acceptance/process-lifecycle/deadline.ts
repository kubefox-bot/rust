import type { BoundedOutcome } from "./types";

export const settleWithin = async <T>(
  pending: Promise<T>,
  timeoutMs: number,
): Promise<BoundedOutcome<T>> => {
  const deadline = Promise.withResolvers<BoundedOutcome<T>>();
  const timer = setTimeout(
    () => deadline.resolve({ kind: "deadline" }),
    timeoutMs,
  );
  try {
    return await Promise.race([
      pending.then(
        (value): BoundedOutcome<T> => ({
          kind: "settled",
          result: { status: "fulfilled", value },
        }),
        (reason): BoundedOutcome<T> => ({
          kind: "settled",
          result: { status: "rejected", reason },
        }),
      ),
      deadline.promise,
    ]);
  } finally {
    clearTimeout(timer);
  }
};
