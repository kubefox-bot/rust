import type { BrowserContracts } from "../../../api";

export type Server = BrowserContracts["servers"][number];
export type Enrollment = BrowserContracts["enrollment"];
export type EnrollmentMode =
  | Readonly<{ kind: "install" }>
  | Readonly<{ kind: "binary" | "recover_identity"; server: Server }>;
export type EnrollmentAttempt = Readonly<{
  operationId: string;
  agentId: string;
  serverId: string;
  profileId: string;
  name: string;
  fingerprint: string;
  packageVersion: string;
  expectedSha256: string;
}>;
export type WizardState =
  | Readonly<{ kind: "target" }>
  | Readonly<{ kind: "trust"; challenge: BrowserContracts["host_key"] }>
  | Readonly<{ kind: "credentials"; challenge: BrowserContracts["host_key"] }>
  | Readonly<{ kind: "submitting"; attempt: EnrollmentAttempt }>
  | Readonly<{
      kind: "operation";
      attempt: EnrollmentAttempt;
      view: Enrollment | null;
    }>;
export type AgentAction = Readonly<{
  kind: "renew" | "revoke";
  server: Server;
}>;
