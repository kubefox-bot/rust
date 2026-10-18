import { DATETIME_INPUT_FRACTION, DATETIME_INPUT_MINUTES } from "./conts";

// The native control supports milliseconds; the canonical cutoff stays in state.
export const displayCutoff = (local: string): string =>
  local.replace(DATETIME_INPUT_FRACTION, "$1");

export const editedCutoff = (local: string): string =>
  DATETIME_INPUT_MINUTES.test(local) ? `${local}:00` : local;
