import type { BrowserContracts } from "./generated/browser.types";
export type ContractName = keyof BrowserContracts;
export type RequestOptions<Name extends ContractName> = Readonly<{
  path: string;
  method?: "GET" | "POST" | "PATCH" | "PUT";
  body?: unknown;
  responseSchema: Name;
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
