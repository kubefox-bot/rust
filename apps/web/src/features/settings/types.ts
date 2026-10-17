import type { BrowserContracts } from "../../api";
export type SettingsSection = "profile" | "agents" | "account";
export type Profile = BrowserContracts["profile"];
export type ProfilePatch = BrowserContracts["save_profile"]["profile"];
