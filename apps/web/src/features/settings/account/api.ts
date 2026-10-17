import { requestEmpty, requestJson } from "../../../api";
import type { BrowserContracts } from "../../../api";

export const sessionSettings = (signal: AbortSignal) =>
  requestJson({
    path: "/settings/session",
    responseSchema: "session_settings",
    signal,
  });

export const saveSessionSettings = (
  input: BrowserContracts["session_settings"],
) =>
  requestJson({
    path: "/settings/session",
    method: "PATCH",
    body: input,
    responseSchema: "session_settings",
  });

export const changePassword = (input: BrowserContracts["change_password"]) =>
  requestEmpty({ path: "/account/password", method: "POST", body: input });

export const prepareRecovery = (input: BrowserContracts["prepare_recovery"]) =>
  requestJson({
    path: "/account/recovery/prepare",
    method: "POST",
    body: input,
    responseSchema: "recovery_candidate",
  });

export const confirmRecovery = (input: BrowserContracts["confirm_recovery"]) =>
  requestEmpty({
    path: "/account/recovery/confirm",
    method: "POST",
    body: input,
  });
