import { expect, test } from "bun:test";
import { mkdtemp, rm, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { publishRelease } from "../publish";
import { promoteRelease } from "../publishing/promote";
import { createDeliveryDescriptor } from "../delivery-descriptor";
import { releaseFixture, sourceSha } from "./delivery-fixture";
import type { PublicationPorts, PublishedRelease } from "../publishing/types";

test("failed lifecycle gate never copies image or creates external release", async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-publish-"));
  try {
    const manifest = releaseFixture();
    await Bun.write(
      join(directory, "release-manifest.json"),
      JSON.stringify(manifest),
    );
    await Bun.write(
      join(directory, "verification.json"),
      JSON.stringify({
        source_sha: sourceSha,
        build: manifest.build,
        image_digest: manifest.image.digest,
        checks: { prelogin_https_health: "FAIL" },
      }),
    );
    const effects: string[] = [];
    const ports: PublicationPorts = {
      copyImage: async () => {
        effects.push("copy");
        return manifest.image.digest;
      },
      createDraft: async () => {
        effects.push("draft");
        return { id: 1, published: false };
      },
      sign: async () => "bundle",
      upload: async () => {},
      verifyAssets: async () => {},
    };
    await expect(
      publishRelease({ directory, sourceSha, sequence: 1, ports }),
    ).rejects.toThrow();
    expect(effects).toEqual([]);
  } finally {
    await rm(directory, { recursive: true });
  }
});
test("late old run cannot promote when main already points to a new SHA", async () => {
  const descriptor = createDeliveryDescriptor({
    manifest: releaseFixture(),
    sourceSha,
    sequence: 1,
    releaseManifestSha256: "1".repeat(64),
  });
  const release: PublishedRelease = {
    sourceSha,
    sequence: 1,
    imageDigest: descriptor.image.digest,
    releaseId: 1,
  };
  let published = false;
  expect(
    await promoteRelease({
      release,
      descriptor,
      ports: {
        currentHead: async () => "b".repeat(40),
        publishedDescriptors: async () => [],
        publishDraft: async () => {
          published = true;
        },
      },
    }),
  ).toBe("superseded");
  expect(published).toBe(false);
});
test("same sequence cannot promote a different image and higher sequence suppresses old candidate", async () => {
  const descriptor = createDeliveryDescriptor({
    manifest: releaseFixture(),
    sourceSha,
    sequence: 3,
    releaseManifestSha256: "1".repeat(64),
  });
  const release: PublishedRelease = {
    sourceSha,
    sequence: 3,
    imageDigest: descriptor.image.digest,
    releaseId: 1,
  };
  const ports = {
    currentHead: async () => sourceSha,
    publishedDescriptors: async () => [
      {
        ...descriptor,
        image: { ...descriptor.image, digest: `sha256:${"9".repeat(64)}` },
      },
    ],
    publishDraft: async () => {
      throw new Error("must not publish");
    },
  };
  await expect(promoteRelease({ release, descriptor, ports })).rejects.toThrow(
    "conflict",
  );
  expect(
    await promoteRelease({
      release,
      descriptor,
      ports: {
        ...ports,
        publishedDescriptors: async () => [{ ...descriptor, sequence: 4 }],
      },
    }),
  ).toBe("superseded");
});
test("promotion checks HEAD again after inspecting channel before exposing complete release", async () => {
  const descriptor = createDeliveryDescriptor({
    manifest: releaseFixture(),
    sourceSha,
    sequence: 1,
    releaseManifestSha256: "1".repeat(64),
  });
  const release: PublishedRelease = {
    sourceSha,
    sequence: 1,
    imageDigest: descriptor.image.digest,
    releaseId: 1,
  };
  let head = sourceSha;
  let published = false;
  expect(
    await promoteRelease({
      release,
      descriptor,
      ports: {
        currentHead: async () => head,
        publishedDescriptors: async () => {
          head = "b".repeat(40);
          return [];
        },
        publishDraft: async () => {
          published = true;
        },
      },
    }),
  ).toBe("superseded");
  expect(published).toBe(false);
});

const verifiedFixture = async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-verified-"));
  const bytes = "image bytes";
  await Bun.write(join(directory, "center.oci.tar"), bytes);
  await mkdir(join(directory, "host-updater"));
  await Bun.write(
    join(directory, "host-updater/astra-updater-linux-amd64.tar.gz"),
    "hostbytes",
  );
  await Bun.write(
    join(directory, "host-updater/package-manifest.json"),
    "hostmanifest",
  );
  const manifest = {
    ...releaseFixture(),
    artifacts: [
      {
        name: "center.oci.tar",
        size: bytes.length,
        sha256: Bun.SHA256.hash(bytes, "hex"),
      },
    ],
  };
  const raw = JSON.stringify(manifest);
  await Bun.write(join(directory, "release-manifest.json"), raw);
  const names = [
    "prelogin_https_health",
    "direct_container_healthcheck",
    "engine_health_state_before_login",
    "static_routes",
    "api_404",
    "credential_login",
    "graceful_sigterm",
    "stopped_backup",
    "recreate_vault_and_ca",
    "restart_revokes_session",
  ];
  await Bun.write(
    join(directory, "verification.json"),
    JSON.stringify({
      build: manifest.build,
      source_sha: sourceSha,
      release_manifest_sha256: Bun.SHA256.hash(raw, "hex"),
      host_package_sha256: Bun.SHA256.hash("hostbytes", "hex"),
      host_manifest_sha256: Bun.SHA256.hash("hostmanifest", "hex"),
      image_digest: manifest.image.digest,
      platform: "linux/amd64",
      checks: Object.fromEntries(names.map((name) => [name, "PASS"])),
    }),
  );
  return { directory, manifest };
};
test("publication copies verified image once, signs exact descriptor and requires all assets", async () => {
  const { directory, manifest } = await verifiedFixture();
  try {
    const effects: string[] = [];
    const ports: PublicationPorts = {
      createDraft: async () => {
        effects.push("draft");
        return { id: 1, published: false };
      },
      copyImage: async () => {
        effects.push("copy");
        return manifest.image.digest;
      },
      sign: async (path) => {
        effects.push("sign");
        expect(JSON.parse(await Bun.file(path).text()).source_sha).toBe(
          sourceSha,
        );
        return join(directory, "delivery.sigstore.json");
      },
      upload: async (_id, files) => {
        effects.push("upload");
        expect(files.map((file) => file.name)).toContain(
          "hmail-center.container",
        );
      },
      verifyAssets: async () => {
        effects.push("verify");
      },
    };
    const release = await publishRelease({
      directory,
      sourceSha,
      sequence: 2,
      ports,
    });
    expect(release.imageDigest).toBe(manifest.image.digest);
    expect(effects).toEqual(["draft", "copy", "sign", "upload", "verify"]);
    const descriptor = await Bun.file(join(directory, "delivery.json")).text();
    effects.length = 0;
    await publishRelease({
      directory,
      sourceSha,
      sequence: 2,
      ports: {
        ...ports,
        createDraft: async () => ({ id: 1, published: true, descriptor }),
      },
    });
    expect(effects).toEqual(["verify"]);
  } finally {
    await rm(directory, { recursive: true });
  }
});
test("changed registry digest prevents signing and missing remote asset leaves release draft", async () => {
  const { directory, manifest } = await verifiedFixture();
  try {
    let signed = false;
    const ports: PublicationPorts = {
      createDraft: async () => ({ id: 1, published: false }),
      copyImage: async () => `sha256:${"8".repeat(64)}`,
      sign: async () => {
        signed = true;
        return join(directory, "delivery.sigstore.json");
      },
      upload: async () => {},
      verifyAssets: async () => {
        throw new Error("missing asset");
      },
    };
    await expect(
      publishRelease({ directory, sourceSha, sequence: 1, ports }),
    ).rejects.toThrow("digest");
    expect(signed).toBe(false);
    await expect(
      publishRelease({
        directory,
        sourceSha,
        sequence: 1,
        ports: { ...ports, copyImage: async () => manifest.image.digest },
      }),
    ).rejects.toThrow("missing asset");
    expect(await Bun.file(join(directory, "publication.json")).exists()).toBe(
      false,
    );
  } finally {
    await rm(directory, { recursive: true });
  }
});
