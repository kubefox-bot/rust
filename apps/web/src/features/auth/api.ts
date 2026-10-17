import { requestEmpty, requestJson } from "../../api";
import type { BrowserContracts } from "../../api";
export const login = (input: BrowserContracts["login"]) =>
  requestJson({
    path: "/auth/login",
    method: "POST",
    body: input,
    responseSchema: "session",
  });
export const logout = () =>
  requestEmpty({ path: "/auth/logout", method: "POST" });
export const session = (signal?: AbortSignal) =>
  requestJson({ path: "/auth/session", responseSchema: "session", signal });
export const activity = (signal?: AbortSignal) =>
  requestEmpty({ path: "/auth/activity", method: "POST", signal });
