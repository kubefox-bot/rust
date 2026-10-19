import {
  ssh_target as sshTargetSchema,
  install as installSchema,
  update as updateSchema,
  enrollments as enrollmentsSchema,
  host_key as hostKeySchema,
  enrollment as enrollmentSchema,
  identity_receipt as identityReceiptSchema,
} from "../../../api/contracts";
import {
  ApiError,
  ContractError,
  decode,
  requestEmpty,
  requestJson,
  type BrowserContracts,
} from "../../../api";
import type { EnrollmentAttempt, Enrollment } from "./types";

export const recent = (signal: AbortSignal) =>
  requestJson({
    path: "/enrollment",
    responseSchema: enrollmentsSchema,
    signal,
  });
export const probe = (
  input: Readonly<{
    target: BrowserContracts["ssh_target"];
    signal: AbortSignal;
  }>,
) =>
  requestJson({
    path: "/enrollment/probe",
    method: "POST",
    body: decode(sshTargetSchema, input.target),
    responseSchema: hostKeySchema,
    signal: input.signal,
  });
export const install = (
  input: Readonly<{ body: BrowserContracts["install"]; signal: AbortSignal }>,
) =>
  requestJson({
    path: "/enrollment/install",
    method: "POST",
    body: decode(installSchema, input.body),
    responseSchema: enrollmentSchema,
    signal: input.signal,
  });
export const update = (
  input: Readonly<{
    agentId: string;
    body: BrowserContracts["update"];
    signal: AbortSignal;
  }>,
) =>
  requestJson({
    path: `/agents/${encodeURIComponent(input.agentId)}/update`,
    method: "POST",
    body: decode(updateSchema, input.body),
    responseSchema: enrollmentSchema,
    signal: input.signal,
  });
export const read = (
  input: Readonly<{ operationId: string; signal: AbortSignal }>,
) =>
  requestJson({
    path: `/enrollment/${encodeURIComponent(input.operationId)}`,
    responseSchema: enrollmentSchema,
    signal: input.signal,
  });
export const reconcile = (
  input: Readonly<{ operationId: string; signal: AbortSignal }>,
) =>
  requestJson({
    path: `/enrollment/${encodeURIComponent(input.operationId)}/reconcile`,
    method: "POST",
    body: {},
    responseSchema: enrollmentSchema,
    signal: input.signal,
  });
export const renew = (
  input: Readonly<{ agentId: string; signal: AbortSignal }>,
) =>
  requestJson({
    path: `/agents/${encodeURIComponent(input.agentId)}/renew`,
    method: "POST",
    body: {},
    responseSchema: identityReceiptSchema,
    signal: input.signal,
  });
export const revoke = (
  input: Readonly<{ agentId: string; signal: AbortSignal }>,
) =>
  requestEmpty({
    path: `/agents/${encodeURIComponent(input.agentId)}/revoke`,
    method: "POST",
    body: {},
    signal: input.signal,
  });
export const verifiedOperation = (
  input: Readonly<{ attempt: EnrollmentAttempt; view: Enrollment }>,
): Enrollment => {
  const { attempt, view } = input;
  if (
    view.id !== attempt.operationId ||
    view.agent_id !== attempt.agentId ||
    view.server_id !== attempt.serverId ||
    view.profile_id !== attempt.profileId ||
    view.fingerprint !== attempt.fingerprint ||
    view.package_version !== attempt.packageVersion ||
    view.expected_sha256 !== attempt.expectedSha256
  ) {
    throw new ContractError();
  }
  return view;
};
export const message = (error: unknown): string =>
  error instanceof ApiError || error instanceof ContractError
    ? error.message
    : "Не удалось получить подтверждение сервера. Проверьте состояние операции.";
