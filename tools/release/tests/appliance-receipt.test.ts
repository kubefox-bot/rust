import { expect, test } from "bun:test";
import { mkdtemp, mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createBundleFixture } from "../../appliance/testing/bundle";
import { releaseFixture, sourceSha } from "./delivery-fixture";
import { writeCiReceipt } from "../ci-receipt";
import { readVerifiedRelease } from "../publishing/gate";
import { REQUIRED_CHECKS } from "../publishing/conts";
test("CI receipt binds appliance and rejects modified ZIP before publication", async () => {
  const directory = await mkdtemp(join(tmpdir(), "astra-receipt-"));
  try {
    const release = {
      ...releaseFixture(),
      artifacts: [
        {
          name: "center.oci.tar",
          size: 5,
          sha256: Bun.SHA256.hash("image", "hex"),
        },
      ],
    };
    await Bun.write(join(directory, "center.oci.tar"), "image");
    await Bun.write(
      join(directory, "release-manifest.json"),
      JSON.stringify(release),
    );
    await mkdir(join(directory, "host-updater"));
    await Bun.write(
      join(directory, "host-updater/astra-updater-linux-amd64.tar.gz"),
      "host",
    );
    await Bun.write(
      join(directory, "host-updater/package-manifest.json"),
      "manifest",
    );
    await createBundleFixture({
      directory: join(directory, "appliance"),
      release,
      sourceSha,
    });
    const run = join(
      directory,
      "delivery-11111111-1111-4111-8111-111111111111",
    );
    await mkdir(run);
    await Bun.write(
      join(run, "delivery-report.json"),
      JSON.stringify({
        build: release.build,
        image_digest: release.image.digest,
        platform: release.image.platform,
        checks: Object.fromEntries(
          REQUIRED_CHECKS.map((name) => [name, "PASS"]),
        ),
      }),
    );
    await writeCiReceipt({ directory, sourceSha });
    await readVerifiedRelease({ directory, sourceSha });
    await Bun.write(
      join(directory, "appliance", `astra-hyperv-${release.build}.zip`),
      "altered",
    );
    await expect(
      readVerifiedRelease({ directory, sourceSha }),
    ).rejects.toThrow();
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
