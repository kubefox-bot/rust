import { ApiError } from "../../../api";
import { read, verifiedOperation, message } from "./api";
import type { Enrollment, EnrollmentAttempt } from "./types";

type AdmissionOutcome =
  | Readonly<{ kind: "rejected"; message: string }>
  | Readonly<{ kind: "accepted"; view: Enrollment }>
  | Readonly<{ kind: "unknown" }>;

export const resolveSubmissionFailure = async (
  input: Readonly<{
    error: unknown;
    attempt: EnrollmentAttempt;
    signal: AbortSignal;
  }>,
): Promise<AdmissionOutcome> => {
  if (!(input.error instanceof ApiError)) return { kind: "unknown" };
  // Validation and CSRF failures precede enrollment reservation and SSH effects.
  if (input.error.status === 400 || input.error.status === 403) {
    return { kind: "rejected", message: message(input.error) };
  }
  if (input.error.status !== 409) return { kind: "unknown" };
  // A conflict can refer to this already admitted operation. Only an explicit
  // missing operation proves that this request did not acquire the durable slot.
  try {
    const view = await read({
      operationId: input.attempt.operationId,
      signal: input.signal,
    });
    return {
      kind: "accepted",
      view: verifiedOperation({ attempt: input.attempt, view }),
    };
  } catch (failure) {
    return failure instanceof ApiError && failure.status === 404
      ? { kind: "rejected", message: message(input.error) }
      : { kind: "unknown" };
  }
};
