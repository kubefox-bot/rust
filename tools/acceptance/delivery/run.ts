import { mkdir, open } from "node:fs/promises";
import { join, resolve } from "node:path";
import { CHECKS, TARGET_PHASES } from "./conts";
import type { CheckResult } from "./types";

// This harness owns full stdout+stderr evidence. The shared process runner returns
// stdout only; invoking it would discard successful test diagnostics from stderr.
const root = resolve(import.meta.dir, "../../..");
const directory = join(root, "target/acceptance/updates", crypto.randomUUID());
await mkdir(directory, { recursive: true, mode: 0o700 });
const results: CheckResult[] = [];
for (const check of CHECKS) {
  const log = join(directory, `${check.name}.log`);
  const file = await open(log, "wx", 0o600);
  const started = performance.now();
  try {
    const child = Bun.spawn([...check.argv], {
      cwd: root,
      stdin: "ignore",
      stdout: file.fd,
      stderr: file.fd,
    });
    const exitCode = await child.exited;
    results.push({
      name: check.name,
      status: exitCode === 0 ? "PASS" : "FAIL",
      exitCode,
      durationMs: performance.now() - started,
      log,
    });
  } finally {
    await file.close();
  }
}
const report = {
  format_version: 1,
  scope: "local-source-checks",
  checks: results,
  target_acceptance: TARGET_PHASES.map((name) => ({ name, status: "NOT RUN" })),
};
await Bun.write(
  join(directory, "report.json"),
  `${JSON.stringify(report, null, 2)}\n`,
);
console.log(`Delivery report: ${join(directory, "report.json")}`);
if (results.some((result) => result.status === "FAIL")) process.exitCode = 1;
