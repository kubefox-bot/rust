type RunOptions = Readonly<{
  argv: readonly string[];
  cwd: string;
  env?: Readonly<Record<string, string | undefined>>;
  inherit?: boolean;
}>;

export const run = async ({
  argv,
  cwd,
  env,
  inherit = false,
}: RunOptions): Promise<string> => {
  const child = Bun.spawn([...argv], {
    cwd,
    env,
    stdin: "inherit",
    stdout: inherit ? "inherit" : "pipe",
    stderr: inherit ? "inherit" : "pipe",
  });
  const [code, output, error] = await Promise.all([
    child.exited,
    child.stdout ? new Response(child.stdout).text() : Promise.resolve(""),
    child.stderr ? new Response(child.stderr).text() : Promise.resolve(""),
  ]);
  if (code !== 0)
    throw new Error(
      `${argv[0]} ${argv[1] ?? ""} failed (${code})${error ? `: ${error.trim()}` : ""}`,
    );
  return output;
};
