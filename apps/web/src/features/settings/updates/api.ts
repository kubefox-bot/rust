import { update_policy as updatePolicySchema } from "../../../api/contracts";
import { decode, requestJson } from "../../../api";
import type { BrowserContracts } from "../../../api";

export const readPolicy = (signal: AbortSignal) =>
  requestJson({
    path: "/settings/updates",
    responseSchema: updatePolicySchema,
    signal,
  });
export const savePolicy = (
  policy: BrowserContracts["update_policy"],
  signal: AbortSignal,
) =>
  requestJson({
    path: "/settings/updates",
    method: "PATCH",
    body: decode(updatePolicySchema, policy),
    responseSchema: updatePolicySchema,
    signal,
  });
