import { open, rename } from "node:fs/promises";

export const writeAtomic = async ({
  path,
  value,
  mode,
}: Readonly<{ path: string; value: string; mode: number }>): Promise<void> => {
  const staged = `${path}.new`;
  const file = await open(staged, "wx", mode);
  try {
    await file.writeFile(value);
    await file.sync();
  } catch (error) {
    try {
      await file.close();
    } catch (closeError) {
      throw new AggregateError(
        [error, closeError],
        "Atomic file write and close failed",
      );
    }
    throw error;
  }
  await file.close();
  await rename(staged, path);
};
