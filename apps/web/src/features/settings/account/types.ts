import type { BrowserContracts } from "../../../api";

export type SessionSettingsState =
  | Readonly<{ kind: "loading" }>
  | Readonly<{ kind: "failed"; message: string }>
  | Readonly<{
      kind: "ready";
      minutes: number;
      saving: boolean;
      saved: boolean;
      error: string;
    }>;

export type RecoveryState =
  | Readonly<{ kind: "password" }>
  | Readonly<{ kind: "preparing" }>
  | Readonly<{
      kind: "display";
      candidate: BrowserContracts["recovery_candidate"];
    }>
  | Readonly<{ kind: "confirm" | "confirming"; candidateId: string }>;
