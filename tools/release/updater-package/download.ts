import { downloadVerified } from "../../lib/download";
import { PACKAGE_DOWNLOAD_LIMIT, PACKAGE_DOWNLOAD_TIMEOUT_MS } from "./conts";
export const downloadPinned = (input: Readonly<{
  url: string; sha256: string; path: string;
}>): Promise<void> => downloadVerified({
  ...input, maxBytes: PACKAGE_DOWNLOAD_LIMIT, timeoutMs: PACKAGE_DOWNLOAD_TIMEOUT_MS,
});
