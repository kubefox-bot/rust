import { resolve } from "node:path";
export const ROOT = resolve(import.meta.dir, "../../../..");
export const STATIC_DIRECTORY = resolve(ROOT, "apps/web/dist");
export const FIXTURE_BINARY = resolve(
  ROOT,
  "target/debug/examples/browser_fixture",
);
export const FIXTURE_STARTUP_MS = 30000;
export const FIXTURE_SHUTDOWN_MS = 10000;
export const MAX_IPC_BYTES = 65536;
