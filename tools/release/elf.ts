import {
  ELF_HEADER_BYTES,
  ELF_MACHINE_OFFSET,
  ELF_MAGIC,
  RUST_TARGETS,
} from "./conts";
import type { Platform } from "./types";

export const requireElfTarget = ({
  bytes,
  platform,
}: Readonly<{
  bytes: Uint8Array;
  platform: Platform;
}>): void => {
  if (
    bytes.length < ELF_HEADER_BYTES ||
    ELF_MAGIC.some((value, index) => bytes[index] !== value)
  )
    throw new Error("Center executable must be a 64-bit little-endian ELF");
  const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (
    header.getUint16(ELF_MACHINE_OFFSET, true) !==
    RUST_TARGETS[platform].elfMachine
  )
    throw new Error(
      "Center ELF architecture does not match the selected release platform",
    );
};
