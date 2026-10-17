import { requestJson } from "../api";
import type { BrowserContracts } from "../api";
import { AGENT_REFRESH_MS } from "./conts";
export const mountFooter = (
  onAgents: (agents: BrowserContracts["agents"] | null) => void,
) => {
  const controller = new AbortController();
  const element = document.querySelector("[data-agents-footer]");
  let pending = false;
  const refresh = async () => {
    if (pending || controller.signal.aborted) return;
    pending = true;
    try {
      const agents = await requestJson({
        path: "/agents",
        responseSchema: "agents",
        signal: controller.signal,
      });
      if (controller.signal.aborted) return;
      onAgents(agents);
      const online = agents.filter(
        (agent) => agent.health.online && !agent.health.stale,
      ).length;
      const versions = [
        ...new Set(
          agents.flatMap((agent) =>
            agent.health.version ? [agent.health.version] : [],
          ),
        ),
      ];
      if (element)
        element.textContent = `Агенты · ${online} из ${agents.length} на связи${versions.length ? ` · ${versions.join(", ")}` : " · ожидание свежих данных"}`;
    } catch {
      if (!controller.signal.aborted) onAgents(null);
      if (!controller.signal.aborted && element)
        element.textContent = "Агенты · не удалось обновить состояние";
    } finally {
      pending = false;
    }
  };
  const timer = window.setInterval(() => {
    refresh().catch(() => {
      if (element) element.textContent = "Агенты · ошибка связи";
    });
  }, AGENT_REFRESH_MS);
  refresh().catch(() => {
    if (element) element.textContent = "Агенты · ошибка связи";
  });
  return {
    refresh,
    dispose: () => {
      controller.abort();
      clearInterval(timer);
      if (element)
        element.textContent = "Агенты · состояние доступно после входа";
    },
  };
};
