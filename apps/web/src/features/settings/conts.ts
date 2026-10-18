import type { SettingsSection } from "./types";
export const SECTIONS = [
  ["profile", "Профиль сервера"],
  ["agents", "Агенты"],
  ["account", "Учётная запись"],
] as const satisfies ReadonlyArray<readonly [SettingsSection, string]>;
export const MYSQL_PORT_MAX = 65535;
export const MYSQL_TLS_OPTIONS = [
  { value: "preferred", label: "По возможности" },
  { value: "required", label: "Обязательно" },
  { value: "disabled_local", label: "Локальное подключение без TLS" },
] as const;
