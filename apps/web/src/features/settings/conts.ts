import type { SettingsSection } from "./types";
export const SECTIONS = [
  ["profile", "Профиль сервера"],
  ["agents", "Агенты"],
  ["account", "Учётная запись"],
] as const satisfies ReadonlyArray<readonly [SettingsSection, string]>;
export const MYSQL_PORT_MAX = 65535;
