import { expect, test } from "bun:test";
import { mkdtemp, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { parseReleaseOptions, pinnedImage } from "../config";
import { sourceFingerprint } from "../source-identity";
import { deployment } from "../deployment";
import { artifact, validateAgentPackage } from "../manifest";
import { inspectOciArchive } from "../oci-archive";
import { stageBuildContext } from "../build-context";
import { initialize } from "../delivery/initialize";
import { centerElfLayer } from "./elf-fixture";
import {
  containerHealth,
  podmanHealthArguments,
  waitForContainerHealth,
} from "../delivery/health";

test("release defaults target Podman Linux amd64 and reject ambiguous deployment configuration", () => {
  expect(parseReleaseOptions({})).toMatchObject({
    engine: "podman",
    platform: "linux/amd64",
    allowDirty: false,
  });
  expect(
    parseReleaseOptions({
      HMAIL_RELEASE_ENGINE: "docker",
      HMAIL_RELEASE_DOCKER_CONTEXT: "colima",
      HMAIL_RELEASE_PLATFORM: "linux/arm64",
      HMAIL_RELEASE_ALLOW_DIRTY: "1",
    }),
  ).toMatchObject({
    engine: "docker",
    context: "colima",
    platform: "linux/arm64",
    allowDirty: true,
  });
  for (const environment of [
    { HMAIL_RELEASE_ENGINE: "unknown" },
    { HMAIL_RELEASE_PLATFORM: "linux/386" },
    { HMAIL_RELEASE_ALLOW_DIRTY: "yes" },
    { HMAIL_RELEASE_IMAGE: "example.org/image;untrusted" },
    { HMAIL_BUILD_VERSION: "invalid build" },
    { HMAIL_BUILD_VERSION: ".." },
  ])
    expect(() => parseReleaseOptions(environment)).toThrow();
});

test("base images and deployment selections require immutable SHA-256 references", () => {
  expect(
    pinnedImage(`docker.io/library/rust@sha256:${"a".repeat(64)}`),
  ).toContain("@sha256:");
  for (const image of [
    "rust:latest",
    "rust:1.99.1",
    "rust@sha256:short",
    `rust@sha256:${"A".repeat(64)}`,
  ])
    expect(() => pinnedImage(image)).toThrow();
});

test("source identity ignores local build output and changes when actual source changes", async () => {
  const root = await mkdtemp(join(tmpdir(), "hmail-source-fingerprint-"));
  try {
    await mkdir(join(root, "apps/web/src"), { recursive: true });
    await mkdir(join(root, "apps/web/dist"), { recursive: true });
    await mkdir(join(root, ".superpowers"), { recursive: true });
    await Bun.write(
      join(root, "apps/web/src/example.ts"),
      "export const source = 1;",
    );
    const before = await sourceFingerprint(root);
    await Bun.write(join(root, "apps/web/dist/generated.js"), "build output");
    await Bun.write(join(root, ".superpowers/progress.log"), "progress output");
    expect(await sourceFingerprint(root)).toBe(before);
    await Bun.write(
      join(root, "apps/web/src/example.ts"),
      "export const source = 2;",
    );
    expect(await sourceFingerprint(root)).not.toBe(before);
  } finally {
    await rm(root, { recursive: true });
  }
});

test("init and Quadlet run select the same immutable image, named volume and non-root ownership", async () => {
  const image = `localhost/hmail-center@sha256:${"b".repeat(64)}`;
  const result = await deployment({ image, build: "0.1.0+0123456789ab.1" });
  expect(result.init).toEqual([
    "podman",
    "run",
    "--rm",
    "-it",
    "--read-only",
    "--user",
    "10001:10001",
    "--userns",
    "keep-id:uid=10001,gid=10001",
    "--volume",
    "hmail-center-data:/data:Z,U",
    image,
    "center",
    "init",
    "--data-dir",
    "/data",
  ]);
  expect(result.container).toContain(`Image=${image}\n`);
  expect(result.container).toContain(
    "Volume=hmail-center-data.volume:/data:Z,U\n",
  );
  expect(result.container).toContain("User=10001\nGroup=10001\n");
  expect(result.container).toContain("UserNS=keep-id:uid=10001,gid=10001\n");
  expect(result.container).toContain("ReadOnly=true\n");
  expect(result.container).toContain("WantedBy=default.target\n");
  expect(result.container).not.toContain("AutoUpdate=");
  expect(result.container).not.toContain("podman.sock");
  expect(result.volume).toContain("VolumeName=hmail-center-data\n");
  const healthField = result.container
    .split("\n")
    .find((line) => line.startsWith("HealthCmd="));
  if (!healthField) throw new Error("Generated Quadlet has no health command");
  const healthCommand: unknown = JSON.parse(
    healthField.slice("HealthCmd=".length),
  );
  expect(healthCommand).toEqual([
    "/usr/local/bin/hmail-cleaner",
    "center",
    "health",
    "--data-dir",
    "/data",
    "--expect-build",
    "0.1.0+0123456789ab.1",
  ]);
});

test("release artifacts bind actual bytes and reject agent packages from another common build", async () => {
  const directory = await mkdtemp(join(tmpdir(), "hmail-delivery-manifest-"));
  try {
    const path = join(directory, "center.oci.tar");
    await Bun.write(path, "fixture image bytes");
    const first = await artifact({ path, name: "center.oci.tar" });
    expect(first.size).toBe(19);
    await Bun.write(path, "changed image bytes");
    expect((await artifact({ path, name: "center.oci.tar" })).sha256).not.toBe(
      first.sha256,
    );
    await Bun.write(
      join(directory, "hmail-cleaner-agent.exe"),
      "MZfixturebinary",
    );
    await Bun.write(
      join(directory, "manifest.json"),
      JSON.stringify({
        product: "hmail-cleaner-agent",
        build: "0.1.0+0123456789ab.1",
        target: "x86_64-pc-windows-msvc",
        protocol_min: 1,
        protocol_max: 1,
        sha256:
          "2ae5908b5b6d60c1c2f3d57151c1aa2e0549b20dc42f99c54d600947cccaff29",
      }),
    );
    await expect(
      validateAgentPackage({ directory, build: "0.1.0+0123456789ab.1" }),
    ).resolves.toMatchObject({ build: "0.1.0+0123456789ab.1" });
    await expect(
      validateAgentPackage({ directory, build: "0.1.0+0123456789ab.2" }),
    ).rejects.toThrow();
    await Bun.write(join(directory, "hmail-cleaner-agent.exe"), "MZtampered");
    await expect(
      validateAgentPackage({ directory, build: "0.1.0+0123456789ab.1" }),
    ).rejects.toThrow();
  } finally {
    await rm(directory, { recursive: true });
  }
});

test("OCI inspection verifies content-addressed metadata and requires the Rust-only runtime contract", async () => {
  const directory = await mkdtemp(join(tmpdir(), "hmail-oci-inspect-"));
  try {
    const center = await centerElfLayer(62);
    const config = JSON.stringify({
      config: {
        User: "10001:10001",
        Entrypoint: ["/usr/local/bin/hmail-cleaner"],
        Healthcheck: {
          Test: [
            "CMD",
            "/usr/local/bin/hmail-cleaner",
            "center",
            "health",
            "--data-dir",
            "/data",
          ],
        },
        Labels: {
          "org.opencontainers.image.version": "0.1.0+0123456789ab.1",
          "org.opencontainers.image.revision": "0123456789ab",
        },
      },
      architecture: "amd64",
      os: "linux",
    });
    const configDigest = Bun.SHA256.hash(config, "hex");
    const manifest = JSON.stringify({
      schemaVersion: 2,
      config: { digest: `sha256:${configDigest}` },
      layers: [{ digest: `sha256:${center.digest}` }],
    });
    const digest = Bun.SHA256.hash(manifest, "hex");
    const archive = join(directory, "center.oci.tar");
    await Bun.write(
      archive,
      new Bun.Archive({
        "oci-layout": JSON.stringify({ imageLayoutVersion: "1.0.0" }),
        "index.json": JSON.stringify({
          manifests: [{ digest: `sha256:${digest}` }],
        }),
        [`blobs/sha256/${digest}`]: manifest,
        [`blobs/sha256/${configDigest}`]: config,
        [`blobs/sha256/${center.digest}`]: center.bytes,
      }),
    );
    const inspected = await inspectOciArchive({
      path: archive,
      build: "0.1.0+0123456789ab.1",
      revision: "0123456789ab",
      platform: "linux/amd64",
      producer: "docker",
    });
    expect(inspected).toMatchObject({
      digest: `sha256:${digest}`,
      configDigest: `sha256:${configDigest}`,
    });
    await expect(
      inspectOciArchive({
        path: archive,
        build: "different-build",
        revision: "0123456789ab",
        platform: "linux/amd64",
        producer: "docker",
      }),
    ).rejects.toThrow();
    const badLayer = await new Bun.Archive({
      "usr/local/bin/bun": "unexpected runtime",
    }).bytes();
    const layerDigest = Bun.SHA256.hash(badLayer, "hex");
    const badManifest = JSON.stringify({
      schemaVersion: 2,
      config: { digest: `sha256:${configDigest}` },
      layers: [{ digest: `sha256:${layerDigest}` }],
    });
    const badDigest = Bun.SHA256.hash(badManifest, "hex");
    await Bun.write(
      archive,
      new Bun.Archive({
        "oci-layout": JSON.stringify({ imageLayoutVersion: "1.0.0" }),
        "index.json": JSON.stringify({
          manifests: [{ digest: `sha256:${badDigest}` }],
        }),
        [`blobs/sha256/${badDigest}`]: badManifest,
        [`blobs/sha256/${configDigest}`]: config,
        [`blobs/sha256/${layerDigest}`]: badLayer,
      }),
    );
    await expect(
      inspectOciArchive({
        path: archive,
        build: "0.1.0+0123456789ab.1",
        revision: "0123456789ab",
        platform: "linux/amd64",
        producer: "docker",
      }),
    ).rejects.toThrow("Runtime image contains");
    await Bun.write(
      archive,
      new Bun.Archive({
        "oci-layout": JSON.stringify({ imageLayoutVersion: "1.0.0" }),
        "index.json": JSON.stringify({
          manifests: [{ digest: `sha256:${digest}` }],
        }),
        [`blobs/sha256/${digest}`]: "tampered metadata",
      }),
    );
    await expect(
      inspectOciArchive({
        path: archive,
        build: "0.1.0+0123456789ab.1",
        revision: "0123456789ab",
        platform: "linux/amd64",
        producer: "docker",
      }),
    ).rejects.toThrow();
  } finally {
    await rm(directory, { recursive: true });
  }
});

test("the Docker and Podman build context includes source while excluding credentials and local metadata", async () => {
  const root = await mkdtemp(join(tmpdir(), "hmail-build-context-"));
  const output = await mkdtemp(join(tmpdir(), "hmail-build-output-"));
  try {
    const files = [
      "Cargo.toml",
      "Cargo.lock",
      "package.json",
      "bun.lock",
      "crates/example/src/lib.rs",
      "apps/web/package.json",
      "apps/web/tsconfig.json",
      "apps/web/astro.config.mjs",
      "apps/web/src/component.svelte",
      "deploy/Containerfile",
      ".containerignore",
      ".env",
      ".git/private",
      ".superpowers/private",
      "apps/web/src/private.env",
      "crates/example/src/.env.production",
    ];
    for (const name of files) {
      await mkdir(join(root, name, ".."), { recursive: true });
      await Bun.write(
        join(root, name),
        name.endsWith(".rs") ? "source" : "fixture",
      );
    }
    await Bun.write(
      join(root, "deploy/Containerfile"),
      await Bun.file(
        resolve(import.meta.dir, "../../../deploy/Containerfile"),
      ).text(),
    );
    const bases = {
      bun: `docker.io/oven/bun@sha256:${"a".repeat(64)}`,
      rust: `docker.io/library/rust@sha256:${"b".repeat(64)}`,
      runtime: `gcr.io/distroless/cc-debian12@sha256:${"c".repeat(64)}`,
      buildkit: `docker.io/moby/buildkit@sha256:${"d".repeat(64)}`,
    };
    const context = await stageBuildContext({ root, directory: output, bases });
    const containerfile = await Bun.file(
      join(context, "deploy/Containerfile"),
    ).text();
    expect(containerfile).toContain(`ARG BUN_IMAGE=${bases.bun}\n`);
    expect(containerfile).toContain(`ARG RUST_IMAGE=${bases.rust}\n`);
    expect(containerfile).toContain(`ARG RUNTIME_IMAGE=${bases.runtime}\n`);
    expect(
      containerfile.split("\n").filter((line) => line.startsWith("FROM ")),
    ).toEqual([
      "FROM --platform=$BUILDPLATFORM ${BUN_IMAGE} AS web",
      "FROM --platform=$BUILDPLATFORM ${RUST_IMAGE} AS rust",
      "FROM ${RUNTIME_IMAGE}",
    ]);
    expect(
      await Bun.file(join(context, "crates/example/src/lib.rs")).text(),
    ).toBe("source");
    for (const name of [
      ".env",
      ".git/private",
      ".superpowers/private",
      "apps/web/src/private.env",
      "crates/example/src/.env.production",
    ])
      expect(await Bun.file(join(context, name)).exists()).toBe(false);
    expect(await Bun.file(join(context, ".dockerignore")).text()).toBe(
      "fixture",
    );
    expect(await Bun.file(join(context, ".containerignore")).text()).toBe(
      "fixture",
    );
    const template = await Bun.file(join(root, "deploy/Containerfile")).text();
    for (const [name, source] of [
      ["missing", template.replace("ARG BUN_IMAGE\n", "")],
      [
        "duplicate",
        template.replace("ARG BUN_IMAGE\n", "ARG BUN_IMAGE\nARG BUN_IMAGE\n"),
      ],
    ]) {
      const directory = join(output, name);
      await mkdir(directory);
      await Bun.write(join(root, "deploy/Containerfile"), source);
      await expect(
        stageBuildContext({ root, directory, bases }),
      ).rejects.toThrow("exactly one ARG BUN_IMAGE");
    }
  } finally {
    await rm(root, { recursive: true });
    await rm(output, { recursive: true });
  }
});

test("OCI metadata rejects a missing or shell-based healthcheck in the distroless runtime", async () => {
  const directory = await mkdtemp(join(tmpdir(), "hmail-oci-health-"));
  try {
    const center = await centerElfLayer(62);
    for (const healthcheck of [
      undefined,
      {
        Test: [
          "CMD-SHELL",
          "/usr/local/bin/hmail-cleaner center health --data-dir /data",
        ],
      },
    ]) {
      const config = JSON.stringify({
        config: {
          User: "10001:10001",
          Entrypoint: ["/usr/local/bin/hmail-cleaner"],
          Labels: {
            "org.opencontainers.image.version": "0.1.0+0123456789ab.1",
            "org.opencontainers.image.revision": "0123456789ab",
          },
          ...(healthcheck === undefined ? {} : { Healthcheck: healthcheck }),
        },
        architecture: "amd64",
        os: "linux",
      });
      const configDigest = Bun.SHA256.hash(config, "hex");
      const manifest = JSON.stringify({
        schemaVersion: 2,
        config: { digest: `sha256:${configDigest}` },
        layers: [{ digest: `sha256:${center.digest}` }],
      });
      const digest = Bun.SHA256.hash(manifest, "hex");
      const path = join(directory, "center.oci.tar");
      await Bun.write(
        path,
        new Bun.Archive({
          "oci-layout": JSON.stringify({ imageLayoutVersion: "1.0.0" }),
          "index.json": JSON.stringify({
            manifests: [{ digest: `sha256:${digest}` }],
          }),
          [`blobs/sha256/${digest}`]: manifest,
          [`blobs/sha256/${configDigest}`]: config,
          [`blobs/sha256/${center.digest}`]: center.bytes,
        }),
      );
      if (healthcheck === undefined) {
        await expect(
          inspectOciArchive({
            path,
            build: "0.1.0+0123456789ab.1",
            revision: "0123456789ab",
            platform: "linux/amd64",
            producer: "podman",
          }),
        ).resolves.toMatchObject({ digest: `sha256:${digest}` });
      } else {
        await expect(
          inspectOciArchive({
            path,
            build: "0.1.0+0123456789ab.1",
            revision: "0123456789ab",
            platform: "linux/amd64",
            producer: "podman",
          }),
        ).rejects.toThrow("direct healthcheck");
      }
      await expect(
        inspectOciArchive({
          path,
          build: "0.1.0+0123456789ab.1",
          revision: "0123456789ab",
          platform: "linux/amd64",
          producer: "docker",
        }),
      ).rejects.toThrow("direct healthcheck");
    }
  } finally {
    await rm(directory, { recursive: true });
  }
});

test("the initialization driver owns a real terminal and confirms the newly issued recovery key", async () => {
  const script = `import {createInterface} from 'node:readline'; const lines=createInterface({input:process.stdin})[Symbol.asyncIterator](); const ask=async(text)=>{process.stdout.write(text);return(await lines.next()).value}; if(await ask('Логин суперадмина: ')!=='fixture-admin')process.exit(1);if(await ask('Пароль: ')!=='fixture-password')process.exit(2);if(await ask('Повторите пароль: ')!=='fixture-password')process.exit(3);const key='hmc-recovery-v1:'+'A'.repeat(43);console.log(key);if(await ask('Введите сохранённый recovery key для подтверждения: ')!==key)process.exit(4);process.exit(0);`;
  await initialize({
    argv: ["bun", "-e", script],
    cwd: import.meta.dir,
    login: "fixture-admin",
    password: "fixture-password",
  });
});

test("initialization failures expose only exit/prompt diagnostics and retain no private terminal text", async () => {
  const error: unknown = await initialize({
    argv: [
      process.execPath,
      "-e",
      'console.log("PRIVATE_TERMINAL_SENTINEL"); process.exit(9);',
    ],
    cwd: import.meta.dir,
    login: "fixture-admin",
    password: "fixture-password",
  }).catch((error: unknown) => error);
  expect(error instanceof Error).toBe(true);
  const message = error instanceof Error ? error.message : "missing failure";
  expect(message).toContain("exit 9; completed prompts 0/4");
  expect(message.includes("PRIVATE_TERMINAL_SENTINEL")).toBe(false);
});

test("engine health validation rejects shell commands, stopped containers and unhealthy state", async () => {
  const config = {
    Healthcheck: {
      Test: [
        "CMD",
        "/usr/local/bin/hmail-cleaner",
        "center",
        "health",
        "--data-dir",
        "/data",
      ],
    },
  };
  expect(
    containerHealth({
      value: [
        {
          Config: config,
          State: { Running: true, Health: { Status: "starting" } },
        },
      ],
    }),
  ).toBe("starting");
  expect(
    containerHealth({
      value: [
        {
          Config: config,
          State: { Running: true, Health: { Status: "healthy" } },
        },
      ],
    }),
  ).toBe("healthy");
  expect(() =>
    containerHealth({
      value: [
        {
          Config: { Healthcheck: { Test: ["CMD-SHELL", "center health"] } },
          State: { Running: true, Health: { Status: "healthy" } },
        },
      ],
    }),
  ).toThrow("direct healthcheck");
  expect(() =>
    containerHealth({
      value: [
        {
          Config: config,
          State: { Running: false, Health: { Status: "healthy" } },
        },
      ],
    }),
  ).toThrow("stopped");
  await expect(
    waitForContainerHealth({
      inspect: async () => [
        {
          Config: config,
          State: { Running: true, Health: { Status: "unhealthy" } },
        },
      ],
    }),
  ).rejects.toThrow("unhealthy");
  const args = podmanHealthArguments("0.1.0+0123456789ab.1");
  const rawCommand = args[args.indexOf("--health-cmd") + 1];
  if (!rawCommand)
    throw new Error("Podman run must configure a health command");
  const test: unknown = JSON.parse(rawCommand);
  expect(test).toEqual([
    "/usr/local/bin/hmail-cleaner",
    "center",
    "health",
    "--data-dir",
    "/data",
    "--expect-build",
    "0.1.0+0123456789ab.1",
  ]);
  if (
    !Array.isArray(test) ||
    !test.every((argument) => typeof argument === "string")
  )
    throw new Error("Podman health command must be native argv");
  const inspected = [
    {
      Config: { Healthcheck: { Test: ["CMD", ...test] } },
      State: { Running: true, Health: { Status: "healthy" } },
    },
  ];
  expect(
    containerHealth({
      value: inspected,
      expectedBuild: "0.1.0+0123456789ab.1",
    }),
  ).toBe("healthy");
  expect(() =>
    containerHealth({ value: inspected, expectedBuild: "different-build" }),
  ).toThrow("direct healthcheck");
});
