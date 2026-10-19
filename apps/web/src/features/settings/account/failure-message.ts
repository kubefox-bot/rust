import { ApiError } from "../../../api";

export const failureMessage = (failure: unknown, fallback: string): string =>
  failure instanceof ApiError ? failure.message : fallback;
