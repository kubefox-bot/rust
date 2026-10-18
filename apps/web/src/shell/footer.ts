import { requestJson } from "../api";
import { availablePackages } from "../agents";
import type { BrowserContracts } from "../api";
import { AGENT_REFRESH_MS } from "./conts";
import { footerView } from "./footer/view";
export const mountFooter = (
  onAgents: (agents: BrowserContracts["agents"] | null) => void,
) => {
  const controller = new AbortController();
  const view = footerView();
  let pending = false;
  let packages: BrowserContracts["packages"] | null = null;
  let packagesPending = false;
  let latestAgents: BrowserContracts["agents"] | null = null;
  const refreshPackages = async () => {
    if (packages !== null || packagesPending || controller.signal.aborted)
      return;
    packagesPending = true;
    try {
      const available = await availablePackages(controller.signal);
      if (controller.signal.aborted) return;
      packages = available;
      if (latestAgents) view.showAgents({ agents: latestAgents, packages });
    } catch {
      // An unavailable catalog leaves update eligibility unknown; retry on the next poll.
      if (!controller.signal.aborted) packages = null;
    } finally {
      packagesPending = false;
    }
  };
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
      latestAgents = agents;
      onAgents(agents);
      view.showAgents({ agents, packages });
    } catch {
      if (!controller.signal.aborted) {
        latestAgents = null;
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
    refreshPackages().catch(reportFailure);
  }, AGENT_REFRESH_MS);
  refresh().catch(reportFailure);
  refreshPackages().catch(reportFailure);
  return {
    refresh,
    dispose: () => {
      controller.abort();
      clearInterval(timer);
      view.dispose();
    },
  };
};
