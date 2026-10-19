import { session as sessionSchema } from "../../api/contracts";
import { requestEmpty, requestJson } from "../../api";
import type { BrowserContracts } from "../../api";
export const login = (input: BrowserContracts["login"]) =>
  requestJson({
    path: "/auth/login",
    method: "POST",
    body: input,
    responseSchema: sessionSchema,
  });
export const logout = () =>
  requestEmpty({ path: "/auth/logout", method: "POST" });
export const session = (signal?: AbortSignal) =>
  requestJson({ path: "/auth/session", responseSchema: sessionSchema, signal });
export const activity = (signal?: AbortSignal) =>
  requestEmpty({ path: "/auth/activity", method: "POST", signal });
