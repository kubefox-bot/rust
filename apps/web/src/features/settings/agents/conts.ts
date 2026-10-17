import type { BrowserContracts } from "../../../api";
import type { EnrollmentMode } from "./types";

export const SSH_PORT = 22;
export const AGENT_PORT = 9443;
export const MAX_PORT = 65535;
export const OPERATION_REFRESH_MS = 2000;
export const ENROLLMENT_PHASES = {
  preparing: "Подготовка подключения",
  transferring: "Передача пакета",
  identity_issued: "Сертификат выпущен",
  installing: "Установка службы агента",
  verifying: "Проверка связи с агентом",
  ready: "Агент установлен",
  failed: "Операция не завершена",
  unknown: "Требуется проверка состояния",
} as const satisfies Record<BrowserContracts["enrollment"]["phase"], string>;
export const MODE_LABELS = {
  install: { title: "Добавить сервер", submit: "Установить агента" },
  binary: { title: "Обновление агента", submit: "Обновить агента" },
  recover_identity: {
    title: "Восстановление доверия",
    submit: "Восстановить доверие",
  },
} as const satisfies Record<
  EnrollmentMode["kind"],
  Readonly<{ title: string; submit: string }>
>;
