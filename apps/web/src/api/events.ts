import { API_PREFIX, MAX_EVENT_BYTES } from "./conts";
import { decode } from "./decode";
import {
  server_id as serverIdSchema,
  job_event as jobEventSchema,
  session as sessionSchema,
} from "./contracts";
import type { BrowserContracts } from "./generated/browser.types";
import type { EventDependencies, EventTransport, Subscription } from "./types";
import { requestJson } from "./request";

type Active = {
  readonly serverId: string;
  readonly transport: EventTransport;
  readonly listeners: Set<Subscription>;
  last: Readonly<{ epoch: string; revision: bigint }> | null;
  checking: boolean;
};
export const createSubscriptions = (dependencies: EventDependencies) => {
  let active: Active | null = null;
  const stop = (owner: Active) => {
    owner.transport.close();
    if (active === owner) active = null;
  };
  const subscribe = (options: Subscription): (() => void) => {
    decode(serverIdSchema, options.serverId);
    if (active && active.serverId !== options.serverId)
      throw new Error("Release previous server subscription first");
    if (!active) {
      const owner: Active = {
        serverId: options.serverId,
        transport: dependencies.open(
          `${API_PREFIX}/events?server_id=${encodeURIComponent(options.serverId)}`,
        ),
        listeners: new Set(),
        last: null,
        checking: false,
      };
      active = owner;
      const resync = () => {
        owner.last = null;
        for (const listener of owner.listeners) listener.onResync();
      };
      owner.transport.addEventListener("resync", resync);
      owner.transport.addEventListener("snapshot", (event) => {
        if (active !== owner) return;
        const decodeEvent = (): BrowserContracts["job_event"] | null => {
          if (
            !(event instanceof MessageEvent) ||
            typeof event.data !== "string" ||
            new TextEncoder().encode(event.data).length > MAX_EVENT_BYTES
          )
            return null;
          try {
            const input: unknown = JSON.parse(event.data);
            return decode(jobEventSchema, input);
          } catch {
            return null;
          }
        };
        const snapshot = decodeEvent();
        if (!snapshot) {
          resync();
          return;
        }
        if (snapshot.job.intent.server_id !== owner.serverId) return;
        const revision = BigInt(snapshot.revision);
        if (owner.last && owner.last.epoch !== snapshot.stream_epoch) {
          resync();
          owner.last = { epoch: snapshot.stream_epoch, revision };
          return;
        }
        if (owner.last && revision <= owner.last.revision) return;
        owner.last = { epoch: snapshot.stream_epoch, revision };
        for (const listener of owner.listeners) listener.onEvent(snapshot);
      });
      owner.transport.addEventListener("error", () => {
        if (owner.checking || active !== owner) return;
        owner.checking = true;
        dependencies
          .authenticated()
          .then((authenticated) => {
            if (active !== owner) return;
            if (!authenticated) {
              stop(owner);
              for (const listener of owner.listeners) listener.onUnauthorized();
            } else resync();
          })
          .catch(() => {
            if (active === owner) resync();
          })
          .finally(() => {
            owner.checking = false;
          });
      });
    }
    const owner = active;
    const listener = { ...options };
    owner.listeners.add(listener);
    let disposed = false;
    return () => {
      if (disposed) return;
      disposed = true;
      owner.listeners.delete(listener);
      if (owner.listeners.size === 0) stop(owner);
    };
  };
  return { subscribe };
};
let shared: ReturnType<typeof createSubscriptions> | null = null;
export const subscribe = (options: Subscription): (() => void) => {
  shared ??= createSubscriptions({
    open: (url) => new EventSource(url),
    authenticated: async () =>
      (
        await requestJson({
          path: "/auth/session",
          responseSchema: sessionSchema,
        })
      ).authenticated,
  });
  return shared.subscribe(options);
};
