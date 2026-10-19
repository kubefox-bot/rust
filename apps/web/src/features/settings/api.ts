import {
  server_id as serverIdSchema,
  save_profile as saveProfileSchema,
  servers as serversSchema,
  cleanup_context as cleanupContextSchema,
  profile as profileSchema,
  profile_version as profileVersionSchema,
  profile_validation as profileValidationSchema,
} from "../../api/contracts";
import { decode, requestJson } from "../../api";
import type { BrowserContracts } from "../../api";
export const servers = (signal: AbortSignal) =>
  requestJson({ path: "/servers", responseSchema: serversSchema, signal });
export const context = (id: string, signal: AbortSignal) =>
  requestJson({
    path: `/servers/${decode(serverIdSchema, id)}/cleanup-context`,
    responseSchema: cleanupContextSchema,
    signal,
  });
export const profile = (id: string, signal: AbortSignal) =>
  requestJson({
    path: `/servers/${decode(serverIdSchema, id)}/profile`,
    responseSchema: profileSchema,
    signal,
  });
export const save = (
  id: string,
  input: BrowserContracts["save_profile"],
  signal: AbortSignal,
) =>
  requestJson({
    path: `/servers/${decode(serverIdSchema, id)}/profile`,
    method: "PATCH",
    body: decode(saveProfileSchema, input),
    responseSchema: profileVersionSchema,
    signal,
  });
export const validate = (id: string, signal: AbortSignal) =>
  requestJson({
    path: `/servers/${decode(serverIdSchema, id)}/validate`,
    method: "POST",
    responseSchema: profileValidationSchema,
    signal,
  });
