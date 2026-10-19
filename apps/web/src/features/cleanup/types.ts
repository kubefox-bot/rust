import type { BrowserContracts } from "../../api";
export type Job = BrowserContracts["job"];
export type Context = BrowserContracts["cleanup_context"];
export type RunParameters = Readonly<{
  scan: Job["intent"]["scan"];
  servicePolicy: Job["intent"]["service_policy"];
  retention: BrowserContracts["retention"];
  limits: Readonly<Job["intent"]["limits"]>;
}>;
export type ConfirmedRun = Readonly<{
  serverId: string;
  serverName: string;
  profileVersion: string;
  previewJobId: string;
  resolvedCutoff: Readonly<BrowserContracts["cutoff"]>;
  scan: RunParameters["scan"];
  servicePolicy: RunParameters["servicePolicy"];
  limits: RunParameters["limits"];
}>;
export type JournalEntry = Readonly<{
  key: string;
  at: string;
  text: string;
  failed: boolean;
}>;
export type Dispatch =
  | Readonly<{ kind: "settled" }>
  | Readonly<{
      kind: "sending" | "unknown";
      intent: BrowserContracts["submit"];
    }>;
export type CleanupState = Readonly<{
  context: Omit<Context, "latest">;
  parameters: RunParameters;
  preview: Job | null;
  previewCandidateId: string | null;
  job: Job | null;
  confirmation: ConfirmedRun | null;
  dispatch: Dispatch;
  journal: ReadonlyArray<JournalEntry>;
  expanded: boolean;
}>;
export type CleanupEvent =
  | Readonly<{ type: "parameters"; parameters: RunParameters }>
  | Readonly<{ type: "context"; context: Context }>
  | Readonly<{ type: "job"; job: Job }>
  | Readonly<{ type: "dispatch"; intent: BrowserContracts["submit"] }>
  | Readonly<{ type: "uncertain" }>
  | Readonly<{ type: "confirm" }>
  | Readonly<{ type: "dismiss" }>
  | Readonly<{ type: "expanded"; expanded: boolean }>;
