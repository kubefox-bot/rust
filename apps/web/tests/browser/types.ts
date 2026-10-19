import type { BrowserContracts } from "../../src/api";
export type GlobalListenerCounts = Readonly<{
  resize: number;
  scroll: number;
  outside: number;
}>;
export type FixtureReady = Readonly<{
  origin: string;
  spki: string;
  servers: readonly [
    BrowserContracts["servers"][number],
    BrowserContracts["servers"][number],
  ];
}>;
export type AgentStats = Readonly<{
  id: string;
  puts: number;
  cancels: number;
  subscriptions: number;
}>;
export type AgentReceiptObservation = Readonly<{
  phase: BrowserContracts["phase"];
  revision: BrowserContracts["counter"];
  lookupHidden: boolean;
  lookups: number;
}>;
export type JobEventObservation = Readonly<{
  at: number;
  event: "open" | "resync" | "snapshot" | "error" | "close";
  jobId?: string;
  phase?: string;
  slotHeld?: boolean;
  revision?: string;
}>;
declare global {
  interface Window {
    readonly __mailGlobalListeners?: () => GlobalListenerCounts;
    readonly __mailEventSources?: Readonly<{ active: number; created: number }>;
    readonly __mailJobEvents?: ReadonlyArray<JobEventObservation>;
  }
}
