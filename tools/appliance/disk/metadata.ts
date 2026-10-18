export const requireMetadata = (
  output: string,
  kind: "file" | "directory",
): void => {
  const fields = new Map(
    output
      .trim()
      .split("\n")
      .map((line) => {
        const [name, value] = line.split(": ");
        return [name, value] as const;
      }),
  );
  const expected = kind === "file" ? 0o100644 : 0o40755;
  if (
    Number(fields.get("st_uid")) !== 0 ||
    Number(fields.get("st_gid")) !== 0 ||
    Number(fields.get("st_mode")) !== expected
  )
    throw new Error(
      `Invalid root ownership or public payload ${kind} permissions`,
    );
};
