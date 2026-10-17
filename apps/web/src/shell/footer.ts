import { requestJson } from "../api";
import type { BrowserContracts } from "../api";
import { AGENT_REFRESH_MS } from "./conts";
import { footerView } from "./footer/view";
export const mountFooter = (
  onAgents: (agents: BrowserContracts["agents"] | null) => void,
) => {
  const controller = new AbortController();
  const view = footerView();
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
      view.showAgents(agents);
    } catch {
      if (!controller.signal.aborted) {
        onAgents(null);
        view.showMessage("Агенты · не удалось обновить состояние");
      }
    } finally {
      pending = false;
    }
  };
  const reportFailure = () => {
    if (!controller.signal.aborted) view.showMessage("Агенты · ошибка связи");
  };
  const timer = window.setInterval(() => {
    refresh().catch(reportFailure);
  }, AGENT_REFRESH_MS);
  refresh().catch(reportFailure);
  return {
    refresh,
    dispose: () => {
      controller.abort();
      clearInterval(timer);
      view.showMessage("Агенты · состояние доступно после входа");
    },
  };
};
