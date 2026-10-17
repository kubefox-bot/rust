import { decode, requestJson } from "../../api";
import type { BrowserContracts } from "../../api";
export const servers = (signal?: AbortSignal) =>
  requestJson({ path: "/servers", responseSchema: "servers", signal });
export const context = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/servers/${decode("server_id", id)}/cleanup-context`,
    responseSchema: "cleanup_context",
    signal,
  });
export const validate = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/servers/${decode("server_id", id)}/validate`,
    method: "POST",
    responseSchema: "profile_validation",
    signal,
  });
export const resolve = (
  id: string,
  retention: BrowserContracts["retention"],
  signal?: AbortSignal,
) =>
  requestJson({
    path: `/servers/${decode("server_id", id)}/resolve`,
    method: "POST",
    body: decode("retention", retention),
    responseSchema: "cutoff",
    signal,
  });
export const submit = (
  input: BrowserContracts["submit"],
  signal?: AbortSignal,
) =>
  requestJson({
    path: `/servers/${input.server_id}/jobs`,
    method: "POST",
    body: decode("submit", input),
    responseSchema: "job",
    signal,
  });
export const reconcile = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/jobs/${id}/reconcile`,
    method: "POST",
    responseSchema: "job",
    signal,
  });
export const cancel = (id: string, signal?: AbortSignal) =>
  requestJson({
    path: `/jobs/${id}/cancel`,
    method: "POST",
    responseSchema: "job",
    signal,
  });
