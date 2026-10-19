import { ssh_target as sshTargetSchema } from "../../../api/contracts";
import { decode, type BrowserContracts } from "../../../api";
import { MAX_PORT } from "./conts";

export const sshTarget = (
  input: Readonly<{ host: string; port: number | undefined }>,
): BrowserContracts["ssh_target"] => {
  const target = decode(sshTargetSchema, input);
  if (
    !target.host ||
    target.host.trim() !== target.host ||
    target.port < 1 ||
    target.port > MAX_PORT
  )
    throw new Error("Укажите хост SSH и порт от 1 до 65535.");
  return target;
};
export const agentEndpoint = (
  input: Readonly<{ value: string; host: string }>,
): string => {
  const endpoint = new URL(input.value);
  const host = endpoint.hostname.replace(/^\[|\]$/g, "");
  if (
    endpoint.protocol !== "https:" ||
    host !== input.host ||
    endpoint.username ||
    endpoint.password ||
    endpoint.pathname !== "/" ||
    endpoint.search ||
    endpoint.hash
  ) {
    throw new Error(
      "Адрес API должен использовать HTTPS и тот же хост, что SSH, без пути и реквизитов.",
    );
  }
  return endpoint.origin;
};
