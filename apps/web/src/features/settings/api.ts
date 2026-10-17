import { decode, requestJson } from "../../api";
import type { BrowserContracts } from "../../api";
export const servers = (signal: AbortSignal) =>
  requestJson({ path: "/servers", responseSchema: "servers", signal });
export const context = (id: string, signal: AbortSignal) =>
  requestJson({
    path: `/servers/${decode("server_id", id)}/cleanup-context`,
    responseSchema: "cleanup_context",
    signal,
  });
export const profile = (id: string, signal: AbortSignal) =>
  requestJson({
    path: `/servers/${decode("server_id", id)}/profile`,
    responseSchema: "profile",
    signal,
  });
export const save = (
  id: string,
  input: BrowserContracts["save_profile"],
  signal: AbortSignal,
) =>
  requestJson({
    path: `/servers/${decode("server_id", id)}/profile`,
    method: "PATCH",
    body: decode("save_profile", input),
    responseSchema: "profile_version",
    signal,
  });
export const validate = (id: string, signal: AbortSignal) =>
  requestJson({
    path: `/servers/${decode("server_id", id)}/validate`,
    method: "POST",
    responseSchema: "profile_validation",
    signal,
  });
