import type { StreamCapture } from "./types";

export const captureStream = ({
  stream,
  write,
}: Readonly<{
  stream: ReadableStream<Uint8Array>;
  write: (chunk: Uint8Array) => number | Promise<number>;
}>): StreamCapture => {
  const reader = stream.getReader();
  let active = true;
  const done = (async () => {
    try {
      while (active) {
        const chunk = await reader.read();
        if (chunk.done) return;
        if (active) await write(chunk.value);
      }
    } finally {
      active = false;
      reader.releaseLock();
    }
  })();
  return {
    done,
    close: async () => {
      if (!active) return;
      active = false;
      await reader.cancel();
    },
  };
};
