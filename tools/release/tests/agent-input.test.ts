import { expect, test } from "bun:test";
import { mkdtemp, mkdir, rm, symlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { buildAgentPackage } from "../build-agent";
import { parseReleaseOptions } from "../config";
import type { ReleaseConfig } from "../types";

const fixture = async () => {
  const root = await mkdtemp(join(tmpdir(), "astra-agent-input-"));
  const source = join(root, "supplied");
  await mkdir(source);
  const bytes = new Uint8Array(128);
  bytes.set([0x4d, 0x5a]);
  const view = new DataView(bytes.buffer);
  view.setUint32(0x3c, 64, true);
  bytes.set([0x50, 0x45, 0, 0], 64);
  view.setUint16(68, 0x8664, true);
  const sha256 = Bun.SHA256.hash(bytes, "hex");
  await Bun.write(join(source, "hmail-cleaner-agent.exe"), bytes);
  await Bun.write(
    join(source, "hmail-cleaner-agent.exe.sha256"),
    `${sha256}  hmail-cleaner-agent.exe\n`,
  );
  await Bun.write(
    join(source, "manifest.json"),
    JSON.stringify({
      product: "hmail-cleaner-agent",
      build: "git-" + "a".repeat(40),
      target: "x86_64-pc-windows-msvc",
      protocol_min: 1,
      protocol_max: 1,
      sha256,
    }),
  );
  const config: ReleaseConfig = {
    root,
    directory: join(root, "release"),
    build: "git-" + "a".repeat(40),
    imageTag: "localhost/astra:test",
    source: {
      revision: "a".repeat(12),
      fingerprint: "b".repeat(64),
      dirty: false,
    },
    options: parseReleaseOptions({ HMAIL_RELEASE_AGENT_PACKAGE: source }),
    environment: {},
    bases: { bun: "", rust: "", runtime: "", buildkit: "" },
  };
  return { root, source, config };
};

test("supplied native agent is validated and copied without rebuilding", async () => {
  const { root, source, config } = await fixture();
  try {
    let builds = 0;
    const output = await buildAgentPackage(config, {
      build: async () => {
        builds++;
      },
    });
    expect(builds).toBe(0);
    expect(await Bun.file(join(output, "manifest.json")).text()).toBe(
      await Bun.file(join(source, "manifest.json")).text(),
    );
  } finally {
    await rm(root, { recursive: true });
  }
});

test("supplied agent rejects mismatched build, executable hash, PE machine and symlinks", async () => {
  for (const change of ["build", "hash", "machine", "symlink", "checksum"]) {
    const { root, source, config } = await fixture();
    try {
      if (change === "build" || change === "hash") {
        const path = join(source, "manifest.json");
        const value = await Bun.file(path).json();
        value[change === "build" ? "build" : "sha256"] = "b".repeat(64);
        await Bun.write(path, JSON.stringify(value));
      }
      if (change === "machine") {
        const path = join(source, "hmail-cleaner-agent.exe");
        const bytes = await Bun.file(path).bytes();
        bytes[68] = 0x4c;
        await Bun.write(path, bytes);
      }
      if (change === "symlink") {
        await rm(join(source, "manifest.json"));
        await symlink("/dev/null", join(source, "manifest.json"));
      }
      if (change === "checksum")
        await Bun.write(
          join(source, "hmail-cleaner-agent.exe.sha256"),
          "wrong\n",
        );
      await expect(
        buildAgentPackage(config, {
          build: async () => {
            throw new Error("must not rebuild");
          },
        }),
      ).rejects.toThrow();
    } finally {
      await rm(root, { recursive: true });
    }
  }
});

test("without supplied package existing builder is called exactly once", async () => {
  const { root, source, config } = await fixture();
  try {
    let builds = 0;
    const options = parseReleaseOptions({});
    const local = join(root, "dist/agent/windows-x64");
    await mkdir(join(root, "dist/agent"), { recursive: true });
    const { cp } = await import("node:fs/promises");
    await buildAgentPackage(
      { ...config, options },
      {
        build: async () => {
          builds++;
          await cp(source, local, { recursive: true });
        },
      },
    );
    expect(builds).toBe(1);
  } finally {
    await rm(root, { recursive: true });
  }
});
