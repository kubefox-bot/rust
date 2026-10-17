import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { writeManifest } from "./agent-manifest";

const readManifest = async (directory: string) => {
  const value: unknown = await Bun.file(join(directory, "manifest.json")).json();
  if (typeof value !== "object" || value === null || !("sha256" in value) || typeof value.sha256 !== "string") {
    throw new Error("Invalid generated manifest");
  }
  return value;
};

test("manifest binds the actual agent bytes and protocol to its build", async () => {
  const directory = await mkdtemp(join(tmpdir(), "hmail-agent-manifest-"));
  try {
    const executable = join(directory, "hmail-cleaner-agent.exe");
    await Bun.write(executable, "MZfixturebinary");
    await writeManifest({ directory, build: "0.1.0+abc123.1" });
    const first = await readManifest(directory);
    expect(first).toMatchObject({ product: "hmail-cleaner-agent", build: "0.1.0+abc123.1", target: "x86_64-pc-windows-msvc", protocol_min: 1, protocol_max: 1 });
    expect(first.sha256).toBe("2ae5908b5b6d60c1c2f3d57151c1aa2e0549b20dc42f99c54d600947cccaff29");
    await Bun.write(executable, "MZchangedfixture");
    await writeManifest({ directory, build: "0.1.0+abc123.2" });
    const second = await readManifest(directory);
    expect(second.sha256).not.toBe(first.sha256);
    expect(await Bun.file(join(directory, "manifest.json.new")).exists()).toBe(false);
  } finally {
    await rm(directory, { recursive: true });
  }
});

test("missing or non-PE input never produces a release manifest", async () => {
  const directory = await mkdtemp(join(tmpdir(), "hmail-agent-manifest-"));
  try {
    await expect(writeManifest({ directory, build: "0.1.0+abc123.1" })).rejects.toThrow();
    await Bun.write(join(directory, "hmail-cleaner-agent.exe"), "not a PE executable");
    await expect(writeManifest({ directory, build: "0.1.0+abc123.1" })).rejects.toThrow();
    expect(await Bun.file(join(directory, "manifest.json")).exists()).toBe(false);
  } finally {
    await rm(directory, { recursive: true });
  }
});
