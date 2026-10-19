import { mount, unmount } from "svelte";
import { writable } from "svelte/store";
import { footerText } from "../agent-status";
import AgentList from "./AgentList.svelte";
import type { AgentListView } from "./types";

export const footerView = () => {
  const summary = document.querySelector("[data-agents-footer]");
  const message = document.querySelector<HTMLElement>("[data-agents-message]");
  const list = document.querySelector("[data-agents-list]");
  const details = document.querySelector<HTMLDetailsElement>(
    "[data-agents-details]",
  );
  const entries = writable<AgentListView>({ agents: [], packages: null });
  const rendered = list
    ? mount(AgentList, { target: list, props: { entries } })
    : null;
  const showMessage = (text: string) => {
    if (message) {
      message.textContent = text;
      message.hidden = false;
    }
    entries.set({ agents: [], packages: null });
    if (details) {
      details.open = false;
      details.hidden = true;
    }
  };
  const showAgents = (value: AgentListView) => {
    if (!value.agents.length) {
      showMessage(footerText(value.agents));
      return;
    }
    if (message) message.hidden = true;
    if (details) details.hidden = false;
    if (summary) summary.textContent = footerText(value.agents);
    entries.set(value);
  };
  const dispose = () => {
    showMessage("Агенты · состояние доступно после входа");
    if (rendered)
      unmount(rendered).catch((failure: unknown) => {
        console.error(
          "Не удалось завершить отображение списка агентов.",
          failure,
        );
      });
  };
  return { showMessage, showAgents, dispose };
};
