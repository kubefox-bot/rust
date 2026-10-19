import { resolve } from "node:path";
export const REPOSITORY = resolve(import.meta.dir, "../..");
export const BROWSER_TESTS = {
  shell: "tests/shell.browser.test.ts",
  cleanup: "tests/browser/cleanup.test.ts",
  history: "tests/browser/history.test.ts",
  settings: "tests/browser/settings.test.ts",
  enrollment: "tests/browser/enrollment.test.ts",
  account: "tests/browser/account.test.ts",
  "preview-files": "tests/browser/preview-files.test.ts",
  controls: "tests/browser/controls.test.ts",
} as const;
