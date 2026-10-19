import { copyFile, mkdir, lstat, readdir } from "node:fs/promises";
import { join } from "node:path";
import {
  AGENT_EXECUTABLE,
  PRIVATE_DIRECTORY_MODE,
  AGENT_PACKAGE_FILES,
  PE_HEADER_POINTER,
  PE_AMD64_MACHINE,
} from "./conts";
import { validateAgentPackage } from "./manifest";
import { run } from "../lib/process";
import type { ReleaseConfig } from "./types";

const validateFiles = async (directory: string): Promise<void> => {
  const names = await readdir(directory);
  if (
    names.length !== AGENT_PACKAGE_FILES.length ||
    names.some(
      (name) => !AGENT_PACKAGE_FILES.some((expected) => expected === name),
    )
  )
    throw new Error("Agent package contains unexpected files");
  for (const name of AGENT_PACKAGE_FILES) {
    const metadata = await lstat(join(directory, name));
    if (!metadata.isFile() || metadata.size === 0)
      throw new Error("Agent package requires nonempty regular files");
  }
  const file = Bun.file(join(directory, AGENT_EXECUTABLE));
  const header = await file.slice(0, PE_HEADER_POINTER + 4).bytes();
  if (
    header.length < PE_HEADER_POINTER + 4 ||
    header[0] !== 0x4d ||
    header[1] !== 0x5a
  )
    throw new Error("Agent requires a Windows PE executable");
  const offset = new DataView(
    header.buffer,
    header.byteOffset,
    header.byteLength,
  ).getUint32(PE_HEADER_POINTER, true);
  if (offset < PE_HEADER_POINTER + 4 || offset + 6 > file.size)
    throw new Error("Invalid Windows PE header");
  const signature = await file.slice(offset, offset + 6).bytes();
  if (
    signature[0] !== 0x50 ||
    signature[1] !== 0x45 ||
    signature[2] !== 0 ||
    signature[3] !== 0 ||
    new DataView(
      signature.buffer,
      signature.byteOffset,
      signature.byteLength,
    ).getUint16(4, true) !== PE_AMD64_MACHINE
  )
    throw new Error("Agent executable target must be Windows x64");
};

export const buildAgentPackage = async (
  config: ReleaseConfig,
  builder?: Readonly<{ build: () => Promise<void> }>,
): Promise<string> => {
  if (!config.options.agentPackageDirectory) {
    if (builder) await builder.build();
    else
      await run({
        argv: ["sh", "tools/build-exe.sh", "--component", "agent"],
        cwd: config.root,
        env: config.environment,
        inherit: true,
      });
  }
  const source =
    config.options.agentPackageDirectory ??
    join(config.root, "dist/agent/windows-x64");
  await validateFiles(source);
  const manifest = await validateAgentPackage({
    directory: source,
    build: config.build,
  });
  const checksum = (
    await Bun.file(join(source, `${AGENT_EXECUTABLE}.sha256`)).text()
  ).trim();
  if (checksum !== `${manifest.sha256}  ${AGENT_EXECUTABLE}`)
    throw new Error("Agent checksum file does not match executable");
  const destination = join(config.directory, "agent-packages", config.build);
  await mkdir(destination, { recursive: true, mode: PRIVATE_DIRECTORY_MODE });
  for (const name of AGENT_PACKAGE_FILES)
    await copyFile(join(source, name), join(destination, name));
  await validateFiles(destination);
  await validateAgentPackage({ directory: destination, build: config.build });
  return destination;
};
