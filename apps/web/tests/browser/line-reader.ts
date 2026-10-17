import { MAX_IPC_BYTES } from "./config";
export const lineReader = (stream: ReadableStream<Uint8Array>) => {
  const reader = stream.getReader();
  const decoder = new TextDecoder("utf-8", { fatal: true });
  let buffered = "";
  const line = async (): Promise<unknown> => {
    while (true) {
      const newline = buffered.indexOf("\n");
      if (newline >= 0) {
        const text = buffered.slice(0, newline);
        buffered = buffered.slice(newline + 1);
        const value: unknown = JSON.parse(text);
        return value;
      }
      const chunk = await reader.read();
      if (chunk.done) throw new Error("Fixture exited before responding");
      buffered += decoder.decode(new Uint8Array(chunk.value), { stream: true });
      if (buffered.length > MAX_IPC_BYTES)
        throw new Error("Fixture response exceeded limit");
    }
  };
  return {
    line,
    close: async () => {
      await reader.cancel();
      reader.releaseLock();
    },
  };
};
