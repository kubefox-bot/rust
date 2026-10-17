import {
  ApiError,
  ContractError,
  decode,
  requestEmpty,
  requestJson,
  type BrowserContracts,
} from "../../../api";
import type { EnrollmentAttempt, Enrollment } from "./types";

export const packages = (signal: AbortSignal) =>
  requestJson({
    path: "/enrollment/packages",
    responseSchema: "packages",
    signal,
  });
export const recent = (signal: AbortSignal) =>
  requestJson({ path: "/enrollment", responseSchema: "enrollments", signal });
export const probe = (
  input: Readonly<{
    target: BrowserContracts["ssh_target"];
    signal: AbortSignal;
  }>,
) =>
  requestJson({
    path: "/enrollment/probe",
    method: "POST",
    body: decode("ssh_target", input.target),
    responseSchema: "host_key",
    signal: input.signal,
  });
export const install = (
  input: Readonly<{ body: BrowserContracts["install"]; signal: AbortSignal }>,
) =>
  requestJson({
    path: "/enrollment/install",
    method: "POST",
    body: decode("install", input.body),
    responseSchema: "enrollment",
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
    body: decode("update", input.body),
    responseSchema: "enrollment",
    signal: input.signal,
  });
export const read = (
  input: Readonly<{ operationId: string; signal: AbortSignal }>,
) =>
  requestJson({
    path: `/enrollment/${encodeURIComponent(input.operationId)}`,
    responseSchema: "enrollment",
    signal: input.signal,
  });
export const reconcile = (
  input: Readonly<{ operationId: string; signal: AbortSignal }>,
) =>
  requestJson({
    path: `/enrollment/${encodeURIComponent(input.operationId)}/reconcile`,
    method: "POST",
    body: {},
    responseSchema: "enrollment",
    signal: input.signal,
  });
export const renew = (
  input: Readonly<{ agentId: string; signal: AbortSignal }>,
) =>
  requestJson({
    path: `/agents/${encodeURIComponent(input.agentId)}/renew`,
    method: "POST",
    body: {},
    responseSchema: "identity_receipt",
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
