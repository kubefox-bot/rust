import { open, rename } from "node:fs/promises";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { readBuildVersion } from "./build-version";
import { AGENT_EXECUTABLE, AGENT_PRODUCT, AGENT_TARGET, BUILD_PATTERN, MANIFEST_FILE_MODE, PE_DOS_MAGIC, PROTOCOL_MAX, PROTOCOL_MIN } from "./conts";
import type { AgentManifestInput } from "./types";

export const writeManifest = async ({ directory, build }: AgentManifestInput): Promise<void> => {
  if (!BUILD_PATTERN.test(build)) throw new Error("Invalid build identifier");
  const file = Bun.file(join(directory, AGENT_EXECUTABLE));
  if (!await file.exists() || file.size === 0) throw new Error("Missing agent executable");
  const header = new Uint8Array(await file.slice(0, PE_DOS_MAGIC.length).arrayBuffer());
  if (!PE_DOS_MAGIC.every((value, index) => header[index] === value)) throw new Error("Agent executable must be a Windows PE file");
  const hash = new Bun.CryptoHasher("sha256");
  for await (const chunk of file.stream()) hash.update(chunk);
  const manifest = { product: AGENT_PRODUCT, build, target: AGENT_TARGET, protocol_min: PROTOCOL_MIN, protocol_max: PROTOCOL_MAX, sha256: hash.digest("hex") };
  const staged = join(directory, "manifest.json.new");
  const handle = await open(staged, "wx", MANIFEST_FILE_MODE);
  try {
    await handle.writeFile(`${JSON.stringify(manifest, null, 2)}\n`);
    await handle.sync();
  } catch (error) {
    try {
      await handle.close();
    } catch (closeError) {
      throw new AggregateError([error, closeError], "Manifest write and cleanup failed");
    }
    throw error;
  }
  await handle.close();
  await rename(staged, join(directory, "manifest.json"));
};

if (import.meta.main) {
  const [command, ...args] = Bun.argv.slice(2);
  switch (command) {
    case "version":
      if (args.length !== 0) throw new Error("version accepts no arguments");
      console.log(await readBuildVersion(process.cwd()));
      break;
    case "write": {
      const { values } = parseArgs({ args, options: { directory: { type: "string" }, build: { type: "string" } }, strict: true });
      if (values.directory === undefined || values.build === undefined) throw new Error("directory and build are required");
      await writeManifest({ directory: values.directory, build: values.build });
      break;
    }
    default:
      throw new Error("Expected version or write");
  }
}
