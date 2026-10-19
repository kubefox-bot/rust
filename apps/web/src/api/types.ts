import type { BrowserContracts } from "./generated/browser.types";
import type { ValidateFunction } from "ajv";
export type RequestOptions<Value> = Readonly<{
  path: string;
  method?: "GET" | "POST" | "PATCH" | "PUT";
  body?: unknown;
  responseSchema: ValidateFunction<Value>;
  signal?: AbortSignal;
}>;
export type Subscription = Readonly<{
  serverId: string;
  onEvent: (event: BrowserContracts["job_event"]) => void;
  onResync: () => void;
  onUnauthorized: () => void;
}>;
export type EventTransport = Pick<
  EventTarget,
  "addEventListener" | "removeEventListener"
> & { close: () => void };
export type EventDependencies = Readonly<{
  open: (url: string) => EventTransport;
  authenticated: () => Promise<boolean>;
}>;
