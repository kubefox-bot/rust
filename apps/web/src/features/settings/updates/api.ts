import { decode, requestJson } from "../../../api";
import type { BrowserContracts } from "../../../api";

export const readPolicy = (signal: AbortSignal) =>
  requestJson({
    path: "/settings/updates",
    responseSchema: "update_policy",
    signal,
  });
export const savePolicy = (
  policy: BrowserContracts["update_policy"],
  signal: AbortSignal,
) =>
  requestJson({
    path: "/settings/updates",
    method: "PATCH",
    body: decode("update_policy", policy),
    responseSchema: "update_policy",
    signal,
  });
