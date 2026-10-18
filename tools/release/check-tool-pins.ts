import { resolve } from "node:path";
import pins from "../../deploy/tool-pins.json";
import { artifact } from "./manifest";

export const checkToolPins = async ({
  root,
  cosign,
}: Readonly<{ root: string; cosign: string }>): Promise<void> => {
  const binary = await artifact({ path: cosign, name: "cosign" });
  const platform =
    process.arch === "x64"
      ? "linux_amd64"
      : process.arch === "arm64"
        ? "linux_arm64"
        : undefined;
  if (
    process.platform !== "linux" ||
    !platform ||
    binary.sha256 !== pins.cosign[platform].sha256
  )
    throw new Error("Cosign binary does not match approved Linux tool pin");
  const trust = await artifact({
    path: resolve(root, pins.trusted_root.path),
    name: "trusted-root.json",
  });
  if (trust.sha256 !== pins.trusted_root.sha256)
    throw new Error("Bundled Sigstore trust root does not match approved pin");
};
if (import.meta.main) {
  const cosign = Bun.which("cosign");
  if (!cosign) throw new Error("Pinned Cosign binary is required");
  await checkToolPins({ root: resolve(import.meta.dir, "../.."), cosign });
}
