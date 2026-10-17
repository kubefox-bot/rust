import { decode } from "../api";
export const selectedServer = (): string | null => {
  const value = new URL(window.location.href).searchParams.get("server");
  if (!value) return null;
  try {
    return decode("server_id", value);
  } catch {
    return null;
  }
};
export const selectServer = (id: string): void => {
  decode("server_id", id);
  const url = new URL(window.location.href);
  url.searchParams.set("server", id);
  window.history.replaceState(null, "", url);
  for (const [selector, path] of [
    ["[data-history-link]", "/history/"],
    ["[data-settings-link]", "/settings/"],
  ] as const) {
    const link = document.querySelector<HTMLAnchorElement>(selector);
    if (link) link.href = `${path}?server=${encodeURIComponent(id)}`;
  }
};
