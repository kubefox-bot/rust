import {
  server_id as serverIdSchema,
  retention as retentionSchema,
  submit as submitSchema,
  servers as serversSchema,
  cleanup_context as cleanupContextSchema,
  profile_validation as profileValidationSchema,
  cutoff as cutoffSchema,
  job as jobSchema,
} from "../../api/contracts";
import { decode, requestJson } from "../../api";
import type { BrowserContracts } from "../../api";
export const servers = (signal?: AbortSignal) =>
  requestJson({ path: "/servers", responseSchema: serversSchema, signal });
export const context = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/servers/${decode(serverIdSchema, id)}/cleanup-context`,
    responseSchema: cleanupContextSchema,
    signal,
  });
export const validate = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/servers/${decode(serverIdSchema, id)}/validate`,
    method: "POST",
    responseSchema: profileValidationSchema,
    signal,
  });
export const resolve = (
  id: string,
  retention: BrowserContracts["retention"],
  signal?: AbortSignal,
) =>
  requestJson({
    path: `/servers/${decode(serverIdSchema, id)}/resolve`,
    method: "POST",
    body: decode(retentionSchema, retention),
    responseSchema: cutoffSchema,
    signal,
  });
export const submit = (
  input: BrowserContracts["submit"],
  signal?: AbortSignal,
) =>
  requestJson({
    path: `/servers/${input.server_id}/jobs`,
    method: "POST",
    body: decode(submitSchema, input),
    responseSchema: jobSchema,
    signal,
  });
export const reconcile = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/jobs/${id}/reconcile`,
    method: "POST",
    responseSchema: jobSchema,
    signal,
  });
export const cancel = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/jobs/${id}/cancel`,
    method: "POST",
    responseSchema: jobSchema,
    signal,
  });
