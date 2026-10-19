export const deadline = async <Value>(
  operation: Promise<Value>,
  milliseconds: number,
): Promise<Value> => {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      operation,
      new Promise<never>((_, reject) => {
        timer = setTimeout(
          () => reject(new Error("Resource fixture deadline exceeded")),
          milliseconds,
        );
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
};
