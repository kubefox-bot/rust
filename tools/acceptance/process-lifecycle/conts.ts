import { CHECK_TIMEOUT_MS } from "../conts";
import type { CheckProcessPolicy } from "./types";

export const CHECK_PROCESS_POLICY = {
  timeoutMs: CHECK_TIMEOUT_MS,
  termGraceMs: 2000,
  killGraceMs: 2000,
  drainMs: 1000,
} as const satisfies CheckProcessPolicy;
