import { run } from "../../lib/process";
export type Guest = Readonly<{
  command: (...argv: readonly string[]) => Promise<string>;
}>;
export const withGuest = async <T>({
  path,
  readonly,
  operation,
}: Readonly<{
  path: string;
  readonly: boolean;
  operation: (guest: Guest) => Promise<T>;
}>): Promise<T> => {
  // The daemon intentionally retains stderr; collecting it until EOF would deadlock.
  const listener = Bun.spawn(
    [
      "guestfish",
      "--listen",
      "--format=vhdx",
      ...(readonly ? ["--ro"] : []),
      "-a",
      path,
    ],
    { stdout: "pipe", stderr: "inherit" },
  );
  const [code, output] = await Promise.all([
    listener.exited,
    new Response(listener.stdout).text(),
  ]);
  if (code !== 0) throw new Error("Cannot start isolated guestfs listener");
  const match = /^GUESTFISH_PID=([1-9][0-9]*);/m.exec(output);
  if (!match) throw new Error("Cannot start isolated guestfs session");
  const pid = match[1];
  const guest: Guest = {
    command: async (...argv) =>
      run({ argv: ["guestfish", `--remote=${pid}`, ...argv], cwd: "/" }),
  };
  let failure: Readonly<{ error: unknown }> | undefined = undefined;
  try {
    await guest.command("run");
    return await operation(guest);
  } catch (error) {
    failure = { error };
    throw error;
  } finally {
    try {
      await guest.command("exit");
    } catch (cleanup) {
      if (failure)
        throw new AggregateError(
          [failure.error, cleanup],
          "Guest operation and cleanup failed",
        );
      throw cleanup;
    }
  }
};
