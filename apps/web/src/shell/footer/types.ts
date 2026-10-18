import type { BrowserContracts } from "../../api";

export type AgentListView = Readonly<{
  agents: BrowserContracts["agents"];
  packages: BrowserContracts["packages"] | null;
}>;
