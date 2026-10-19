import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { run } from "../../lib/process";
import { sourceIdentity } from "../../release/source-identity";
import { requireUnchangedSource } from "../inputs";
test("changing source while assembling an appliance prevents a distributable result", async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-source-"));
  try {
    await Bun.write(join(root, "Cargo.toml"), "fixture");
    const git = async (...argv: string[]) =>
      run({ argv: ["git", ...argv], cwd: root });
    await git("init", "--quiet");
    await git("add", "Cargo.toml");
    await git(
      "-c",
      "core.hooksPath=/dev/null",
      "-c",
      "user.name=Fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "-c",
      "commit.gpgsign=false",
      "commit",
      "--quiet",
      "-m",
      "test: source fixture",
    );
    const sourceSha = (await git("rev-parse", "HEAD")).trim();
    const sourceFingerprint = (await sourceIdentity(root)).fingerprint;
    await requireUnchangedSource({ root, sourceSha, sourceFingerprint });
    await Bun.write(join(root, "Cargo.toml"), "changed");
    await expect(
      requireUnchangedSource({ root, sourceSha, sourceFingerprint }),
    ).rejects.toThrow("source changed");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
