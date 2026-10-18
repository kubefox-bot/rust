import { requestJson } from "../api";

export const availablePackages = (signal: AbortSignal) =>
  requestJson({
    path: "/enrollment/packages",
    responseSchema: "packages",
    signal,
  });
