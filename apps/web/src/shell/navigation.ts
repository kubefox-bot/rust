import { decode } from "../api";

type ServerReference = Readonly<{ id: string; slug?: string | null }>;

export const serverUrlValue = (server: ServerReference): string =>
  server.slug || server.id;

export const resolveServerReference = ({
  reference,
  servers,
}: Readonly<{
  reference: string;
  servers: ReadonlyArray<ServerReference>;
}>): string => {
  const selected = servers.find(
    (server) => server.id === reference || server.slug === reference,
  );
  if (!selected)
    throw new Error("Выбранный сервер не найден. Выберите сервер из списка.");
  return selected.id;
};

export const selectedServer = (
  servers: ReadonlyArray<ServerReference>,
): string | null => {
  const reference = new URL(window.location.href).searchParams.get("server");
  return reference ? resolveServerReference({ reference, servers }) : null;
};

export const selectServer = ({
  id,
  servers,
}: Readonly<{ id: string; servers: ReadonlyArray<ServerReference> }>): void => {
  decode("server_id", id);
  const server = servers.find((server) => server.id === id);
  if (!server) throw new Error("Выбранный сервер не найден.");
  const reference = serverUrlValue(server);
  const url = new URL(window.location.href);
  url.searchParams.set("server", reference);
  window.history.replaceState(null, "", url);
  for (const [selector, path] of [
    ["[data-history-link]", "/history/"],
    ["[data-settings-link]", "/settings/"],
    ["[data-cleanup-link]", "/"],
  ] as const) {
    const link = document.querySelector<HTMLAnchorElement>(selector);
    if (link) link.href = `${path}?server=${encodeURIComponent(reference)}`;
  }
};
