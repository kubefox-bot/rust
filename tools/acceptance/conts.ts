import type { Check } from "./types";
export const FIXTURE_MARKER = ".hmail-mvp-fixture.json";
export const FIXTURE_KIND = "synthetic-only";
export const FIXTURE_ROOT_PREFIX = "hmail-mvp-synthetic-";
export const FIXTURE_SERVICE_PREFIX = "hmail-mvp-fixture-";
export const MAX_MARKER_BYTES = 1024;
export const MAX_JOURNAL_ROWS = 1000;
export const MAX_QUEUED_EVENTS = 256;
export const CHECK_TIMEOUT_MS = 30 * 60 * 1000;
export const MYSQL_GUARD_TIMEOUT_MS = 30 * 1000;
export const FIXTURE_DATABASE_URL =
  "mysql://root:hmail_fixture@127.0.0.1:13306/mysql?ssl-mode=disabled";

export const DATABASE_CHECKS: readonly Check[] = [
  { name: "test-db", argv: ["mise", "run", "test:db"] },
  { name: "test-agent-db", argv: ["mise", "run", "test:agent-db"] },
];

export const SOURCE_CHECKS: readonly Check[] = [
  {
    name: "fixture-guards",
    argv: [
      "bun",
      "test",
      "tools/acceptance/tests/scenarios.test.ts",
      "tools/acceptance/tests/process.test.ts",
      "tools/acceptance/tests/mysql-fixture.test.ts",
    ],
  },
  { name: "rust-workspace", argv: ["mise", "run", "check"] },
  {
    name: "center-only-cli",
    argv: [
      "cargo",
      "test",
      "-p",
      "hmail-cleaner",
      "--no-default-features",
      "--features",
      "center",
      "--test",
      "center_cli",
      "--locked",
    ],
  },
  {
    name: "measurement-guards",
    argv: ["bun", "test", "tools/acceptance/tests/measurement.test.ts"],
  },
  { name: "browser-contracts", argv: ["mise", "run", "contracts:check"] },
  { name: "web-types-unit", argv: ["mise", "run", "web:check"] },
  {
    name: "browser-behavior",
    argv: ["bun", "run", "--cwd", "apps/web", "test:browser"],
  },
  {
    name: "delivery-contracts",
    argv: ["bun", "test", "./tools/release/tests"],
  },
];
