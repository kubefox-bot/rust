export type Verdict = "PASS" | "FAIL" | "NOT RUN";
export type Check = Readonly<{ name: string; argv: readonly string[] }>;
export type Receipt = Readonly<{
  name: string;
  verdict: Verdict;
  seconds?: number;
  log?: string;
  reason?: string;
}>;
export type AcceptanceConfig = Readonly<{
  root: string;
  output: string;
  database: boolean;
  deliveryManifest: string | undefined;
  dockerContext: string;
  environment: Readonly<Record<string, string | undefined>>;
}>;
