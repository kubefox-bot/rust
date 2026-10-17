import { decode } from "../../src/api";
import type { AgentStats, FixtureReady } from "./types";
const record = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);
const count = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
export const readyMessage = (value: unknown): FixtureReady => {
  if (
    !record(value) ||
    typeof value.origin !== "string" ||
    typeof value.spki !== "string" ||
    !/^[A-Za-z0-9+/]{43}=$/.test(value.spki)
  )
    throw new Error("Invalid fixture metadata");
  const origin = new URL(value.origin);
  if (
    origin.protocol !== "https:" ||
    origin.hostname !== "localhost" ||
    origin.origin !== value.origin ||
    !origin.port
  )
    throw new Error("Fixture must use local HTTPS");
  const servers = decode("servers", value.servers);
  const [first, second] = servers;
  if (servers.length !== 2 || !first || !second)
    throw new Error("Fixture must contain two servers");
  return { origin: value.origin, spki: value.spki, servers: [first, second] };
};
export const statsMessage = (value: unknown): ReadonlyArray<AgentStats> => {
  if (!record(value) || !Array.isArray(value.stats))
    throw new Error("Invalid fixture stats");
  return value.stats.map((item: unknown) => {
    if (
      !record(item) ||
      !count(item.puts) ||
      !count(item.cancels) ||
      !count(item.subscriptions)
    )
      throw new Error("Invalid agent stats");
    return {
      id: decode("server_id", item.id),
      puts: item.puts,
      cancels: item.cancels,
      subscriptions: item.subscriptions,
    };
  });
};
export const acknowledged = (value: unknown): void => {
  if (!record(value) || value.ok !== true)
    throw new Error("Fixture command failed");
};
