import { copyFile, mkdir, chmod, mkdtemp, rm } from "node:fs/promises";
import { join, dirname } from "node:path";
import { run } from "../../lib/process";
import { artifact } from "../manifest";
import { validateUpdaterBinary, writePackageInventory } from "./inventory";
import {
  HOST_PACKAGE_ARCHIVE,
  HOST_PACKAGE_FILES,
  HOST_PACKAGE_MANIFEST,
  BOOTSTRAP_IDENTITY,
  BOOTSTRAP_ISSUER,
} from "./conts";
import { downloadPinned } from "./download";
import pins from "../../../deploy/tool-pins.json";

export const createUpdaterPackage = async ({
  root,
  binary,
  output,
  sourceSha,
}: Readonly<{
  root: string;
  binary: string;
  output: string;
  sourceSha: string;
}>): Promise<void> => {
  await validateUpdaterBinary(binary);
  const version: unknown = JSON.parse(
    await run({ argv: [binary, "version"], cwd: root }),
  );
  if (
    typeof version !== "object" ||
    version === null ||
    !("version" in version) ||
    version.version !== 1 ||
    !("build" in version) ||
    version.build !== `git-${sourceSha}`
  )
    throw new Error(
      "Host updater binary does not match release source and version",
    );
  const trustPath = join(root, pins.trusted_root.path);
  if (
    (await artifact({ path: trustPath, name: "trusted-root.json" })).sha256 !==
    pins.trusted_root.sha256
  )
    throw new Error("Bundled trust root does not match pinned hash");
  await mkdir(dirname(output), { recursive: true, mode: 0o700 });
  await mkdir(output, { mode: 0o700 });
  const scratch = await mkdtemp(join(dirname(output), ".astra-bootstrap-"));
  try {
    const cosign = join(output, "cosign");
    await downloadPinned({ ...pins.cosign.linux_amd64, path: cosign });
    await chmod(cosign, 0o755);
    const checksums = join(scratch, "cosign_checksums.txt");
    const bundle = join(scratch, "cosign_checksums.sigstore.json");
    await downloadPinned({
      url: pins.cosign.release_checksums_url,
      sha256: pins.cosign.release_checksums_sha256,
      path: checksums,
    });
    await downloadPinned({
      url: pins.cosign.release_checksums_bundle_url,
      sha256: pins.cosign.release_checksums_bundle_sha256,
      path: bundle,
    });
    await run({
      argv: [
        cosign,
        "verify-blob",
        "--trusted-root",
        trustPath,
        "--bundle",
        bundle,
        "--certificate-identity",
        BOOTSTRAP_IDENTITY,
        "--certificate-oidc-issuer",
        BOOTSTRAP_ISSUER,
        checksums,
      ],
      cwd: root,
    });
    const expected = `${pins.cosign.linux_amd64.sha256}  cosign-linux-amd64`;
    if (!(await Bun.file(checksums).text()).split(/\r?\n/).includes(expected))
      throw new Error(
        "Signed upstream checksums do not approve pinned Cosign binary",
      );
    await copyFile(binary, join(output, "astra-updater"));
    await copyFile(trustPath, join(output, "sigstore-trusted-root.json"));
    for (const name of HOST_PACKAGE_FILES) {
      if (
        name === "astra-updater" ||
        name === "cosign" ||
        name === "sigstore-trusted-root.json"
      )
        continue;
      const source = join(root, "deploy/updater", name);
      await artifact({ path: source, name });
      await copyFile(source, join(output, name));
    }
    for (const name of HOST_PACKAGE_FILES)
      await chmod(
        join(output, name),
        name === "astra-updater" || name === "cosign" || name === "install.sh"
          ? 0o755
          : 0o644,
      );
    await writePackageInventory({
      directory: output,
      sourceSha,
      names: HOST_PACKAGE_FILES,
    });
    await run({
      argv: [
        "tar",
        "--sort=name",
        "--mtime=@0",
        "--owner=0",
        "--group=0",
        "--numeric-owner",
        "--format=posix",
        "--pax-option=delete=atime,delete=ctime",
        "-czf",
        join(output, HOST_PACKAGE_ARCHIVE),
        "-C",
        output,
        "--",
        ...HOST_PACKAGE_FILES,
        HOST_PACKAGE_MANIFEST,
        "SHA256SUMS",
      ],
      cwd: root,
    });
    console.log(
      `Host updater bootstrap package: ${join(output, HOST_PACKAGE_ARCHIVE)}`,
    );
  } finally {
    await rm(scratch, { recursive: true });
  }
};
