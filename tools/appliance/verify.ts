import { join } from "node:path";
import { run } from "../lib/process";
import { artifact } from "../release/manifest";
import {
  APPLIANCE_MANIFEST,
  BUNDLE_FILES,
  CHECKSUMS,
  MAX_RELEASE_ASSET_BYTES,
} from "./conts";
import { parseAppliance } from "./contracts";
import { listFiles, requireRelativeFile, verifyInventory } from "./inventory";
import { checksums } from "./inventory";
import type { ApplianceManifest, Artifact } from "./types";
const verifyZipMember = async (zip: string, file: Artifact): Promise<void> => {
  const child = Bun.spawn(["unzip", "-p", zip, file.name], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const error = new Response(child.stderr).text();
  const hash = new Bun.CryptoHasher("sha256");
  let size = 0;
  try {
    for await (const bytes of child.stdout) {
      size += bytes.byteLength;
      if (size > file.size)
        throw new Error("Bundle ZIP member exceeds inventory");
      hash.update(bytes);
    }
    if (
      (await child.exited) !== 0 ||
      size !== file.size ||
      hash.digest("hex") !== file.sha256
    )
      throw new Error(
        `Bundle ZIP member differs: ${file.name}; ${await error}`,
      );
    await error;
  } finally {
    child.kill();
    await child.exited;
  }
};
export const verifyBundle = async ({
  directory,
  sourceSha,
}: Readonly<{
  directory: string;
  sourceSha: string;
}>): Promise<ApplianceManifest> => {
  const manifestPath = await requireRelativeFile(directory, APPLIANCE_MANIFEST);
  const manifest = parseAppliance(await Bun.file(manifestPath).json());
  if (
    manifest.source_sha !== sourceSha ||
    JSON.stringify(manifest.files.map((file) => file.name).sort()) !==
      JSON.stringify([...BUNDLE_FILES].sort())
  )
    throw new Error("Appliance source or inventory differs");
  await verifyInventory({ directory, files: manifest.files });
  const manifestFile = await artifact({
    path: manifestPath,
    name: APPLIANCE_MANIFEST,
  });
  const expectedChecksums = checksums(
    [...manifest.files, manifestFile].sort((a, b) =>
      a.name.localeCompare(b.name),
    ),
  );
  if (
    (await Bun.file(await requireRelativeFile(directory, CHECKSUMS)).text()) !==
    expectedChecksums
  )
    throw new Error("Appliance checksum index differs");
  const report: unknown = await Bun.file(
    join(directory, "assembly-report.json"),
  ).json();
  if (
    typeof report !== "object" ||
    report === null ||
    !("build" in report) ||
    report.build !== manifest.build ||
    !("source_sha" in report) ||
    report.source_sha !== sourceSha ||
    !("checks" in report) ||
    typeof report.checks !== "object" ||
    report.checks === null
  )
    throw new Error("Missing same-source appliance assembly report");
  for (const name of [
    "offline_injection",
    "payload_readback",
    "upstream_unchanged",
    "first_boot_state",
  ])
    if (!(name in report.checks) || Reflect.get(report.checks, name) !== "PASS")
      throw new Error(`Appliance assembly gate failed: ${name}`);
  const ignition = await Bun.file(join(directory, "base.ign")).text();
  if (
    !ignition.includes(
      `ASTRA_PAYLOAD_SHA256=${manifest.payload_checksums_sha256}`,
    )
  )
    throw new Error("Ignition does not bind payload checksum");
  const zipName = `astra-hyperv-${manifest.build}.zip`;
  const names = [...BUNDLE_FILES, APPLIANCE_MANIFEST, CHECKSUMS].sort();
  if (
    JSON.stringify(await listFiles(directory)) !==
    JSON.stringify([...names, zipName].sort())
  )
    throw new Error("Unexpected files in appliance output");
  const zip = await requireRelativeFile(directory, zipName);
  if (
    (await artifact({ path: zip, name: zipName })).size >=
    MAX_RELEASE_ASSET_BYTES
  )
    throw new Error("Appliance ZIP exceeds GitHub release asset limit");
  const members = (await run({ argv: ["unzip", "-Z1", zip], cwd: directory }))
    .trim()
    .split("\n")
    .sort();
  if (JSON.stringify(members) !== JSON.stringify(names))
    throw new Error("Unexpected ZIP inventory");
  for (const file of [
    ...manifest.files,
    manifestFile,
    await artifact({ path: join(directory, CHECKSUMS), name: CHECKSUMS }),
  ])
    await verifyZipMember(zip, file);
  return manifest;
};
