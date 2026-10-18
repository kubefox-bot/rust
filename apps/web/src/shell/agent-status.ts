import type { BrowserContracts } from "../api";
import { formatDateTime } from "../lib/time";
import { AGENT_STATUS_LABELS } from "./conts";
type Agent = BrowserContracts["agents"][number];
export const agentStatus = (agent: Agent): keyof typeof AGENT_STATUS_LABELS => {
  if (agent.revoked || agent.certificate_expired) return "offline";
  if (!agent.health.observed_at) return "pending";
  if (agent.health.stale) return "stale";
  return agent.health.online ? "online" : "offline";
};
export const agentBadge = (agent: Agent | undefined) => {
  if (!agent) return { state: "pending", label: AGENT_STATUS_LABELS.pending };
  if (agent.revoked) return { state: "offline", label: "Доступ отозван" };
  if (agent.certificate_expired)
    return { state: "offline", label: "Сертификат истёк" };
  const state = agentStatus(agent);
  return {
    state,
    label: state === "online" ? "Агент на связи" : AGENT_STATUS_LABELS[state],
  };
};
export const footerText = (agents: BrowserContracts["agents"]): string => {
  if (!agents.length) return "Агенты · не добавлены";
  const counts = { online: 0, stale: 0, offline: 0, pending: 0 };
  for (const agent of agents) counts[agentStatus(agent)]++;
  const versions = [
    ...new Set(
      agents.flatMap((agent) =>
        agent.health.version ? [agent.health.version] : [],
      ),
    ),
  ];
  const observed = agents
    .flatMap((agent) =>
      agent.health.observed_at ? [agent.health.observed_at] : [],
    )
    .sort()
    .at(-1);
  return `Агенты · ${counts.online} на связи · ${counts.stale} устарело · ${counts.offline} недоступен · ${counts.pending} ожидает проверки · ${versions.join(", ") || "версия неизвестна"} · ${observed ? "проверка " + formatDateTime(observed) : "ещё не проверены"}`;
};
