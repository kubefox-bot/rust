type TerminationOptions = Readonly<{
  pid: number;
  termGraceMs: number;
  killGraceMs: number;
}>;
type GroupProbe =
  | Readonly<{ kind: "alive" | "gone" }>
  | Readonly<{ kind: "uncertain"; error: Error }>;

const signalGroup = (pid: number, signal: NodeJS.Signals | 0): boolean => {
  try {
    process.kill(-pid, signal);
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ESRCH")
      return false;
    throw error;
  }
};

const probeGroup = (pid: number): GroupProbe => {
  try {
    return { kind: signalGroup(pid, 0) ? "alive" : "gone" };
  } catch (error) {
    // XNU killpg1 excludes zombie group members and may return EPERM while
    // the group is being reaped. EPERM is uncertainty, never proof of exit.
    // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_sig.c
    if (error instanceof Error && "code" in error && error.code === "EPERM")
      return { kind: "uncertain", error };
    throw error;
  }
};

const waitForGroupExit = async (pid: number, graceMs: number) => {
  const pollIntervalMs = 25;
  const deadline = performance.now() + graceMs;
  while (true) {
    const probe = probeGroup(pid);
    const remainingMs = deadline - performance.now();
    switch (probe.kind) {
      case "gone":
        return true;
      case "alive":
        if (remainingMs <= 0) return false;
        break;
      case "uncertain":
        if (remainingMs <= 0)
          throw new Error(
            `Process group ${pid} exit remains unconfirmed after its bounded grace period`,
            { cause: probe.error },
          );
        break;
      default:
        throw new Error(
          `Unhandled process-group probe: ${probe satisfies never}`,
        );
    }
    await Bun.sleep(Math.min(pollIntervalMs, remainingMs));
  }
};

/** Only for an owned POSIX process group launched with detached:true.
 * Windows process-tree shutdown is unsupported; do not substitute a single PID kill.
 */
export const terminateProcessGroup = async ({
  pid,
  termGraceMs,
  killGraceMs,
}: TerminationOptions): Promise<
  Readonly<{ gone: boolean; escalated: boolean }>
> => {
  if (process.platform === "win32")
    throw new Error("POSIX process-group shutdown is unsupported on Windows");
  if (!Number.isSafeInteger(pid) || pid <= 1)
    throw new Error("An owned positive process-group leader PID is required");
  if (
    !Number.isFinite(termGraceMs) ||
    termGraceMs < 0 ||
    !Number.isFinite(killGraceMs) ||
    killGraceMs < 0
  )
    throw new Error("Process-group grace periods must be finite milliseconds");
  if (!signalGroup(pid, "SIGTERM")) return { gone: true, escalated: false };
  if (await waitForGroupExit(pid, termGraceMs))
    return { gone: true, escalated: false };
  if (!signalGroup(pid, "SIGKILL")) return { gone: true, escalated: true };
  return { gone: await waitForGroupExit(pid, killGraceMs), escalated: true };
};
