import { packages as packagesSchema } from "../api/contracts";
import { requestJson } from "../api";

export const availablePackages = (signal: AbortSignal) =>
  requestJson({
    path: "/enrollment/packages",
    responseSchema: packagesSchema,
    signal,
  });
