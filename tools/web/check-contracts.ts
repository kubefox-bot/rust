import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { generateContracts } from "./generate-contracts";
const temporary = await mkdtemp(join(tmpdir(), "hmail-contracts-"));
try {
  await generateContracts(temporary);
  for (const name of [
    "browser.schema.json",
    "browser.types.ts",
    "validators.js",
    "validators.d.ts",
  ]) {
    const [expected, actual] = await Promise.all([
      readFile(join(temporary, name)),
      readFile(join("apps/web/src/api/generated", name)),
    ]);
    if (!expected.equals(actual))
      throw new Error(`Generated contract is stale: ${name}`);
  }
  console.log("Browser contracts are reproducible.");
} finally {
  await rm(temporary, { recursive: true, force: true });
}
