import { resolve } from "node:path";
import type { AcceptanceConfig } from "./types";
import { FIXTURE_DATABASE_URL } from "./conts";
export const readAcceptanceConfig = (): AcceptanceConfig => {
  const environment = { ...Bun.env };
  const root = resolve(import.meta.dir, "../..");
  const database = environment.HMAIL_MVP_DATABASE ?? "0";
  if (database !== "0" && database !== "1")
    throw new Error("HMAIL_MVP_DATABASE must be 0 or 1");
  const dockerContext = environment.HMAIL_TEST_DOCKER_CONTEXT ?? "colima";
  if (!/^[a-zA-Z0-9_.-]+$/.test(dockerContext))
    throw new Error("Invalid fixture Docker context");
  return {
    root,
    output: resolve(root, "target/acceptance"),
    database: database === "1",
    dockerContext,
    deliveryManifest: environment.HMAIL_MVP_RELEASE_MANIFEST,
    environment: {
      ...environment,
      HMAIL_TEST_DOCKER_CONTEXT: dockerContext,
      HMAIL_TEST_DATABASE_URL: FIXTURE_DATABASE_URL,
    },
  };
};
