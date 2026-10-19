import { constants } from "node:zlib";

export const BROTLI_OPTIONS = {
  params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
} as const;
export const GZIP_OPTIONS = { level: 9 } as const;
export const COMPRESSIBLE_EXTENSIONS: ReadonlySet<string> = new Set([
  ".js",
  ".css",
]);
