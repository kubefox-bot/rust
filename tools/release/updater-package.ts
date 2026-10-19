import { parseArgs } from "node:util";
import { resolve } from "node:path";
import { createUpdaterPackage } from "./updater-package/build";
import { SOURCE_SHA_PATTERN } from "./delivery-descriptor/conts";

if (import.meta.main) {
  const { values } = parseArgs({
    args: Bun.argv.slice(2),
    options: {
      binary: { type: "string" },
      output: { type: "string" },
      "source-sha": { type: "string" },
    },
    strict: true,
  });
  if (
    !values.binary ||
    !values.output ||
    !values["source-sha"] ||
    !SOURCE_SHA_PATTERN.test(values["source-sha"])
  )
    throw new Error("Expected --binary, --output and full --source-sha");
  if (process.platform !== "linux" || process.arch !== "x64")
    throw new Error(
      "Updater bootstrap package must be built and verified on Linux amd64",
    );
  await createUpdaterPackage({
    root: resolve(import.meta.dir, "../.."),
    binary: resolve(values.binary),
    output: resolve(values.output),
    sourceSha: values["source-sha"],
  });
}
