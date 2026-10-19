import { chmod, copyFile, mkdir, mkdtemp, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import compatibility from "../../deploy/compatibility.json";
import { artifact, readReleaseManifest } from "../release/manifest";
import { run } from "../lib/process";
import { readBuildOptions } from "./config";
import { parseAppliance, parseLock } from "./contracts";
import {
  APPLIANCE_DISK,
  APPLIANCE_MANIFEST,
  BUNDLE_FILES,
  CHECKSUMS,
  PAYLOAD_MANIFEST,
} from "./conts";
import { inventoryFiles, listFiles } from "./inventory";
import { assemblePayload } from "./payload";
import { checksums } from "./inventory";
import { generateIgnition } from "./ignition";
import { unpackPinnedZip } from "./upstream";
import { injectPayload } from "./disk";
import type { BuildOptions } from "./types";
import { verifyBundle } from "./verify";
import { requireUnchangedSource } from "./inputs";
export const buildAppliance = async (
  options: BuildOptions,
): Promise<string> => {
  const lock = parseLock(
    await Bun.file(join(options.root, "deploy/appliance/inputs.json")).json(),
  );
  await mkdir(dirname(options.output), { recursive: true, mode: 0o700 });
  await mkdir(options.output, { mode: 0o700 });
  const scratch = await mkdtemp(join(dirname(options.output), ".appliance-"));
  try {
    const payload = join(scratch, "payload");
    const identity = await assemblePayload({ options, directory: payload });
    const payloadFiles = await inventoryFiles({
      directory: payload,
      names: await listFiles(payload),
    });
    const payloadChecksums = await artifact({
      path: join(payload, CHECKSUMS),
      name: CHECKSUMS,
    });
    const payloadManifest = await artifact({
      path: join(payload, PAYLOAD_MANIFEST),
      name: PAYLOAD_MANIFEST,
    });
    const upstream = join(scratch, "upstream.vhdx");
    await unpackPinnedZip({
      ...lock.fcos,
      cache: options.cache,
      output: upstream,
      member: `fedora-coreos-${lock.fcos.release}-hyperv.x86_64.vhdx`,
    });
    if (
      (await artifact({ path: upstream, name: "upstream.vhdx" })).sha256 !==
      lock.fcos.uncompressed_sha256
    )
      throw new Error("Uncompressed upstream VHDX digest mismatch");
    const report = await injectPayload({
      upstream,
      output: join(options.output, APPLIANCE_DISK),
      payload,
      scratch,
      files: payloadFiles,
    });
    if (
      (await artifact({ path: upstream, name: "upstream.vhdx" })).sha256 !==
      lock.fcos.uncompressed_sha256
    )
      throw new Error("Original upstream VHDX was modified");
    await unpackPinnedZip({
      ...lock.kvpctl,
      cache: options.cache,
      output: join(options.output, "kvpctl.exe"),
      member: "kvpctl-amd64.exe",
    });
    for (const name of ["Install-Astra.ps1", "installer-lib.ps1", "README.txt"])
      await copyFile(
        join(options.root, "deploy/appliance", name),
        join(options.output, name),
      );
    await Bun.write(
      join(options.output, "base.ign"),
      `${JSON.stringify(await generateIgnition({ root: options.root, payloadSha256: payloadChecksums.sha256 }), null, 2)}\n`,
    );
    await Bun.write(
      join(options.output, "assembly-report.json"),
      `${JSON.stringify(
        {
          format_version: 1,
          build: identity.build,
          source_sha: identity.source_sha,
          checks: {
            offline_injection: "PASS",
            payload_readback: "PASS",
            upstream_unchanged: "PASS",
            first_boot_state: "PASS",
            native_hyperv: "NOT_RUN",
            two_vm_identity: "NOT_RUN",
          },
          ...report,
        },
        null,
        2,
      )}\n`,
    );
    const release = await readReleaseManifest(
      join(payload, "release-manifest.json"),
    );
    const manifest = parseAppliance({
      format_version: 1,
      product: "astra-hyperv",
      build: identity.build,
      source_sha: identity.source_sha,
      platform: "linux/amd64",
      schema_version: compatibility.current_schema_version,
      image: release.image,
      upstream: lock,
      payload_manifest_sha256: payloadManifest.sha256,
      payload_checksums_sha256: payloadChecksums.sha256,
      files: await inventoryFiles({
        directory: options.output,
        names: BUNDLE_FILES,
      }),
    });
    await Bun.write(
      join(options.output, APPLIANCE_MANIFEST),
      `${JSON.stringify(manifest, null, 2)}\n`,
    );
    await Bun.write(
      join(options.output, CHECKSUMS),
      checksums(
        [
          ...manifest.files,
          await artifact({
            path: join(options.output, APPLIANCE_MANIFEST),
            name: APPLIANCE_MANIFEST,
          }),
        ].sort((a, b) => a.name.localeCompare(b.name)),
      ),
    );
    const zipName = `astra-hyperv-${identity.build}.zip`;
    await run({
      argv: [
        "zip",
        "-q",
        "-6",
        join(options.output, zipName),
        ...BUNDLE_FILES,
        APPLIANCE_MANIFEST,
        CHECKSUMS,
      ],
      cwd: options.output,
    });
    await verifyBundle({
      directory: options.output,
      sourceSha: options.sourceSha,
    });
    await requireUnchangedSource({
      root: options.root,
      sourceSha: options.sourceSha,
      sourceFingerprint: release.source_fingerprint,
    });
    for (const name of [
      ...BUNDLE_FILES,
      APPLIANCE_MANIFEST,
      CHECKSUMS,
      zipName,
    ])
      await chmod(join(options.output, name), 0o600);
    console.log(`Appliance bundle: ${join(options.output, zipName)}`);
    return join(options.output, zipName);
  } finally {
    await rm(scratch, { recursive: true, force: true });
  }
};
if (import.meta.main) await buildAppliance(readBuildOptions());
