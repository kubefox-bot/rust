export type CheckProcessPolicy = Readonly<{
  timeoutMs: number;
  termGraceMs: number;
  killGraceMs: number;
  drainMs: number;
}>;

export type CheckExecutionOptions = Readonly<{
  policy?: CheckProcessPolicy;
  signal?: AbortSignal;
}>;

export type ProcessOutcome =
  | Readonly<{ kind: "exited"; code: number }>
  | Readonly<{ kind: "stopped"; reason: string }>;

export type CheckProcessOptions = Readonly<{
  argv: readonly string[];
  cwd: string;
  environment: Readonly<Record<string, string | undefined>>;
  output: Readonly<{
    stdout: (chunk: Uint8Array) => number | Promise<number>;
    stderr: (chunk: Uint8Array) => number | Promise<number>;
  }>;
  policy: CheckProcessPolicy;
  signal?: AbortSignal;
}>;

export type BoundedOutcome<T> =
  | Readonly<{ kind: "settled"; result: PromiseSettledResult<T> }>
  | Readonly<{ kind: "deadline" }>;

export type StreamCapture = Readonly<{
  done: Promise<void>;
  close: () => Promise<void>;
}>;

export type ProcessCleanupOptions = Readonly<{
  child: Readonly<{ pid: number; exited: Promise<number> }>;
  captures: readonly StreamCapture[];
  policy: CheckProcessPolicy;
}>;

export type CapturedProcessOptions = Omit<CheckProcessOptions, "output">;
