export type CheckResult = Readonly<{
  name: string;
  status: "PASS" | "FAIL";
  exitCode: number;
  durationMs: number;
  log: string;
}>;
