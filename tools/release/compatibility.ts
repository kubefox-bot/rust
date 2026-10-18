import { join, resolve } from "node:path";
import { run } from "../lib/process";
import schema from "../../deploy/delivery.schema.json";

const root = resolve(import.meta.dir, "../..");
const mode = Bun.argv[2] ?? "check";
if (mode !== "check" && mode !== "generate")
  throw new Error("Expected check or generate");
const bytes = await run({
  argv: [
    "cargo",
    "run",
    "--quiet",
    "--locked",
    "-p",
    "hmail-cleaner-protocol",
    "--example",
    "export_delivery_compatibility",
  ],
  cwd: root,
});
const compatibility: unknown = JSON.parse(bytes);
if (
  typeof compatibility !== "object" ||
  compatibility === null ||
  !("current_schema_version" in compatibility) ||
  typeof compatibility.current_schema_version !== "number" ||
  !Number.isSafeInteger(compatibility.current_schema_version) ||
  !("backup_schema_versions" in compatibility) ||
  !Array.isArray(compatibility.backup_schema_versions) ||
  !compatibility.backup_schema_versions.includes(
    compatibility.current_schema_version,
  )
)
  throw new Error("Invalid exported delivery compatibility");
const expected = [
  ["deploy/compatibility.json", JSON.stringify(compatibility, null, 2) + "\n"],
  [
    "deploy/delivery.schema.json",
    JSON.stringify(
      {
        ...schema,
        properties: {
          ...schema.properties,
          schema_version: { const: compatibility.current_schema_version },
        },
      },
      null,
      2,
    ) + "\n",
  ],
] as const;
for (const [name, content] of expected) {
  const path = join(root, name);
  if (mode === "generate") await Bun.write(path, content);
  else if ((await Bun.file(path).text()) !== content)
    throw new Error(`Stale delivery contract: ${name}`);
}
console.log("Delivery contracts are reproducible.");
