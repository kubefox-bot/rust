import type { BrowserContracts } from "../../api";

export type PageRequest = Readonly<{
  cursor: BrowserContracts["counter"] | null;
  previousCursors: ReadonlyArray<BrowserContracts["counter"] | null>;
}>;

export type LoadedPage = Readonly<{
  request: PageRequest;
  data: BrowserContracts["history"];
  receivedAt: string;
}>;

export type HistoryState =
  | Readonly<{ kind: "ready"; value: LoadedPage }>
  | Readonly<{
      kind: "loading";
      request: PageRequest;
      previous: LoadedPage | null;
    }>
  | Readonly<{
      kind: "failed";
      request: PageRequest;
      previous: LoadedPage | null;
      message: string;
    }>;
