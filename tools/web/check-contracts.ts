import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { generateContracts } from "./generate-contracts";
const temporary = await mkdtemp(join(tmpdir(), "hmail-contracts-"));
try {
  await generateContracts(temporary);
  const generated = "apps/web/src/api/generated";
  const [expectedNames, actualNames] = await Promise.all([
    readdir(temporary),
    readdir(generated),
  ]);
  if (expectedNames.sort().join("\n") !== actualNames.sort().join("\n"))
    throw new Error("Generated browser contract files are stale");
  for (const name of expectedNames) {
    const [expected, actual] = await Promise.all([
      readFile(join(temporary, name)),
      readFile(join(generated, name)),
    ]);
    if (!expected.equals(actual))
      throw new Error(`Generated contract is stale: ${name}`);
  }
  console.log("Browser contracts are reproducible.");
} finally {
  await rm(temporary, { recursive: true, force: true });
}
