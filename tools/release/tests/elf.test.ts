import { expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { inspectOciArchive } from "../oci-archive";
import { centerElfLayer } from "./elf-fixture";
import { requireElfTarget } from "../elf";

const header = (machine: number) => {
  const bytes = new Uint8Array(64);
  bytes.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1]);
  new DataView(bytes.buffer).setUint16(16, 3, true);
  new DataView(bytes.buffer).setUint16(18, machine, true);
  return bytes;
};

test("ELF validation accepts each actual target machine and rejects malformed 64-bit headers", () => {
  for (const fixture of [
    { platform: "linux/amd64", machine: 62 },
    { platform: "linux/arm64", machine: 183 },
  ] as const)
    expect(() =>
      requireElfTarget({
        bytes: header(fixture.machine),
        platform: fixture.platform,
      }),
    ).not.toThrow();
  const wrongClass = header(62);
  wrongClass[4] = 1;
  const wrongEndian = header(62);
  wrongEndian[5] = 2;
  for (const bytes of [
    new Uint8Array(64),
    header(62).slice(0, 20),
    wrongClass,
    wrongEndian,
  ])
    expect(() => requireElfTarget({ bytes, platform: "linux/amd64" })).toThrow(
      "ELF",
    );
});

test("OCI architecture labels cannot conceal a center ELF built for the wrong machine", async () => {
  const directory = await mkdtemp(join(tmpdir(), "hmail-elf-target-"));
  try {
    for (const fixture of [
      { platform: "linux/amd64", actual: 183 },
      { platform: "linux/arm64", actual: 62 },
    ] as const) {
      const config = JSON.stringify({
        architecture: fixture.platform.split("/")[1],
        os: "linux",
        config: {
          User: "10001:10001",
          Entrypoint: ["/usr/local/bin/hmail-cleaner"],
          Labels: {
            "org.opencontainers.image.version": "fixture",
            "org.opencontainers.image.revision": "0123456789ab",
          },
        },
      });
      const configDigest = Bun.SHA256.hash(config, "hex");
      const layer = await centerElfLayer(fixture.actual);
      const layerDigest = layer.digest;
      const manifest = JSON.stringify({
        schemaVersion: 2,
        config: { digest: `sha256:${configDigest}` },
        layers: [{ digest: `sha256:${layerDigest}` }],
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
          [`blobs/sha256/${layerDigest}`]: layer.bytes,
        }),
      );
      await expect(
        inspectOciArchive({
          path,
          build: "fixture",
          revision: "0123456789ab",
          platform: fixture.platform,
          producer: "podman",
        }),
      ).rejects.toThrow("ELF");
    }
  } finally {
    await rm(directory, { recursive: true });
  }
});
