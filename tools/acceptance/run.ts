import { mkdir } from "node:fs/promises";
import { join } from "node:path";
import { readAcceptanceConfig } from "./config";
import type { AcceptanceConfig, Check, Receipt } from "./types";
import { DATABASE_CHECKS, SOURCE_CHECKS } from "./conts";
import { executeCheck } from "./execute-check";
import { verifyMysqlFixture } from "./mysql-fixture";
import type { CheckExecutionOptions } from "./process-lifecycle";
export const runAcceptance = async (
  config: AcceptanceConfig,
  { signal, policy }: CheckExecutionOptions = {},
) => {
  await mkdir(config.output, { recursive: true, mode: 0o700 });
  const receipts: Receipt[] = [];
  const execute = (check: Check): Promise<Receipt> =>
    signal?.aborted
      ? Promise.resolve({
          name: check.name,
          verdict: "NOT RUN",
          reason: "Acceptance cancelled before this check",
        })
      : executeCheck(config, check, { signal, policy });
  for (const check of SOURCE_CHECKS) {
    if (!signal?.aborted) console.log(`RUN ${check.name}`);
    const receipt = await execute(check);
    receipts.push(receipt);
    console.log(`${receipt.verdict} ${receipt.name}`);
  }
  if (config.database && !signal?.aborted) {
    try {
      await verifyMysqlFixture(config, { signal, policy });
      for (const check of DATABASE_CHECKS) {
        const receipt = await execute(check);
        receipts.push(receipt);
        console.log(`${receipt.verdict} ${receipt.name}`);
      }
    } catch (error) {
      receipts.push({
        name: "mysql-fixture",
        verdict: "FAIL",
        reason:
          error instanceof Error ? error.message : "Fixture validation failed",
      });
      for (const check of DATABASE_CHECKS)
        receipts.push({
          name: check.name,
          verdict: "NOT RUN",
          reason: "MySQL fixture validation failed before SQL checks",
        });
    }
  } else
    receipts.push({
      name: "mysql-effects",
      verdict: "NOT RUN",
      reason: signal?.aborted
        ? "Acceptance cancelled before MySQL fixture validation"
        : "Set HMAIL_MVP_DATABASE=1 with the named disposable MySQL fixture.",
    });
  if (config.deliveryManifest)
    receipts.push(
      await execute({
        name: "oci-lifecycle",
        argv: ["bun", "tools/release/verify.ts", config.deliveryManifest],
      }),
    );
  else
    receipts.push({
      name: "oci-lifecycle",
      verdict: "NOT RUN",
      reason: "Set HMAIL_MVP_RELEASE_MANIFEST to the local release manifest.",
    });
  receipts.push(
    {
      name: "windows-server-2016-native",
      verdict: "NOT RUN",
      reason:
        "Two synthetic Server 2016 VMs with SCM/DACL/Win32-OpenSSH are not attached to this local harness.",
    },
    {
      name: "rootless-quadlet-logoff-reboot",
      verdict: "NOT RUN",
      reason:
        "Run on the dedicated Linux VM with rootless Podman, systemd user manager and cgroup v2.",
    },
    {
      name: "thirty-minute-slow-consumer",
      verdict: "NOT RUN",
      reason:
        "Bounded queue/journal behavior is covered by tests; a 30-minute wall-clock observation is a separate run.",
    },
  );
  if (signal?.aborted)
    receipts.push({
      name: "acceptance-cancelled",
      verdict: "FAIL",
      reason: "Acceptance cancelled",
    });
  const report = { created_at: new Date().toISOString(), checks: receipts };
  await Bun.write(
    join(config.output, "report.json"),
    JSON.stringify(report, null, 2) + "\n",
  );
  if (receipts.some((receipt) => receipt.verdict === "FAIL"))
    process.exitCode = 1;
  return report;
};
if (import.meta.main) {
  const controller = new AbortController();
  const cancel = () => controller.abort();
  process.on("SIGINT", cancel);
  process.on("SIGTERM", cancel);
  try {
    await runAcceptance(readAcceptanceConfig(), { signal: controller.signal });
  } finally {
    process.off("SIGINT", cancel);
    process.off("SIGTERM", cancel);
  }
}
