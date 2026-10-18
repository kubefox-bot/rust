import { join } from "node:path";
import { chmod, writeFile } from "node:fs/promises";
import type { AcceptanceConfig, Check, Receipt } from "./types";
import {
  CHECK_PROCESS_POLICY,
  executeProcess,
  type CheckExecutionOptions,
} from "./process-lifecycle";

export const executeCheck = async (
  config: AcceptanceConfig,
  check: Check,
  options: CheckExecutionOptions = {},
): Promise<Receipt> => {
  const log = join(config.output, `${check.name}.log`);
  const privateLogMode = 0o600;
  const started = performance.now();
  // Bun 1.4.3 FileSink does not truncate reused files, and Bun.write's empty
  // string overload does not apply its mode option on this supported host.
  const initialized = await writeFile(log, "", { mode: privateLogMode })
    .then(async () => {
      await chmod(log, privateLogMode);
      return Bun.file(log).writer();
    })
    .then(
      (file) => ({ status: "fulfilled", file }) as const,
      (reason: unknown) => ({ status: "rejected", reason }) as const,
    );
  if (initialized.status === "rejected")
    return {
      name: check.name,
      verdict: "FAIL",
      seconds: (performance.now() - started) / 1000,
      log,
      reason:
        initialized.reason instanceof Error
          ? initialized.reason.message
          : "Unknown check log initialization failure",
    };
  const file = initialized.file;
  const write = (chunk: Uint8Array) => file.write(chunk);
  const outcome = await executeProcess({
    argv: check.argv,
    cwd: config.root,
    environment: config.environment,
    output: { stdout: write, stderr: write },
    policy: options.policy ?? CHECK_PROCESS_POLICY,
    signal: options.signal,
  }).then(
    (value) => ({ status: "fulfilled", value }) as const,
    (reason: unknown) => ({ status: "rejected", reason }) as const,
  );
  const closed = await Promise.allSettled([file.end()]);
  const closeFailure = closed.find((result) => result.status === "rejected");
  const reasons = [
    ...(outcome.status === "rejected" ? [outcome.reason] : []),
    ...(closeFailure?.status === "rejected" ? [closeFailure.reason] : []),
  ].map((error) =>
    error instanceof Error ? error.message : "Unknown check execution failure",
  );
  const receipt = {
    name: check.name,
    seconds: (performance.now() - started) / 1000,
    log,
  };
  if (outcome.status === "rejected")
    return { ...receipt, verdict: "FAIL", reason: reasons.join("; ") };
  if (outcome.value.kind === "stopped")
    return {
      ...receipt,
      verdict: "FAIL",
      reason: [outcome.value.reason, ...reasons].join("; "),
    };
  if (reasons.length > 0)
    return { ...receipt, verdict: "FAIL", reason: reasons.join("; ") };
  return {
    ...receipt,
    verdict: outcome.value.code === 0 ? "PASS" : "FAIL",
  };
};
