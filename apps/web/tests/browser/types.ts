import type { BrowserContracts } from "../../src/api";
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
declare global {
  interface Window {
    readonly __mailEventSources?: Readonly<{ active: number; created: number }>;
  }
}
