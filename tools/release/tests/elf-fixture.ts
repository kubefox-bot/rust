export const centerElfLayer = async (machine: number) => {
  const header = new Uint8Array(64);
  header.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1]);
  const view = new DataView(header.buffer);
  view.setUint16(16, 3, true);
  view.setUint16(18, machine, true);
  const bytes = await new Bun.Archive({
    "usr/local/bin/hmail-cleaner": header,
  }).bytes();
  return { bytes, digest: Bun.SHA256.hash(bytes, "hex") };
};
