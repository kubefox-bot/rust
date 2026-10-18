import type { BrowserContracts } from "../../api";
import { agentStatus, footerText } from "../agent-status";
import { AGENT_STATUS_LABELS } from "../conts";

export const footerView = () => {
  const summary = document.querySelector("[data-agents-footer]");
  const message = document.querySelector<HTMLElement>("[data-agents-message]");
  const list = document.querySelector("[data-agents-list]");
  const details = document.querySelector<HTMLDetailsElement>(
    "[data-agents-details]",
  );
  const showMessage = (text: string) => {
    if (message) {
      message.textContent = text;
      message.hidden = false;
    }
    list?.replaceChildren();
    if (details) {
      details.open = false;
      details.hidden = true;
    }
  };
  const showAgents = (agents: BrowserContracts["agents"]) => {
    if (!agents.length) {
      showMessage(footerText(agents));
      return;
    }
    if (message) message.hidden = true;
    if (details) details.hidden = false;
    if (summary) summary.textContent = footerText(agents);
    if (!list) return;
    const rows = document.createDocumentFragment();
    for (const agent of agents) {
      const row = document.createElement("li");
      row.dataset.serverId = agent.server_id;
      const name = document.createElement("strong");
      name.textContent = agent.name;
      const status = document.createElement("span");
      const observedStatus = agentStatus(agent);
      status.dataset.status = observedStatus;
      status.textContent = AGENT_STATUS_LABELS[observedStatus];
      const version = document.createElement("span");
      version.textContent = agent.health.version
        ? `Версия: ${agent.health.version}`
        : "Версия неизвестна";
      const contact = document.createElement("span");
      if (agent.health.observed_at) {
        const time = document.createElement("time");
        time.dateTime = agent.health.observed_at;
        time.textContent = new Date(agent.health.observed_at).toLocaleString(
          "ru-RU",
        );
        contact.append("Последний контакт: ", time);
      } else contact.textContent = "Контакт ещё не получен";
      row.append(name, status, version, contact);
      rows.append(row);
    }
    list.replaceChildren(rows);
  };
  return { showMessage, showAgents };
};
